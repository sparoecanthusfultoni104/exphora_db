//! NAT traversal: STUN-based public address discovery.
//!
//! # What this module does
//! - [`discover_public_addr`]: queries a STUN server over UDP (port 0 = ephemeral)
//!   to learn the **public IP** of the seeder, then combines it with the TCP
//!   listener port to produce a `SocketAddr` suitable for inclusion in the
//!   ShareLink.  The STUN server only sees the UDP packet — no shard data
//!   ever leaves the peer-to-peer TCP channel.
//!
//! - [`try_connect_wan`]: attempts `TcpStream::connect` with a 10-second
//!   timeout.  Works transparently for open/full-cone/restricted-cone NAT
//!   (~60-70% of consumer routers).  Returns a descriptive `Err` for symmetric
//!   NAT / CGNAT, telling the user to enable port forwarding.
//!
//! # Architecture note (honest)
//! This is **not** true simultaneous TCP hole punching — that requires
//! coordinated signalling so both peers connect at the same instant.  The
//! current model (seeder listens, fetcher connects later) is client-server;
//! what we gain from STUN is the seeder's real WAN address instead of
//! `0.0.0.0`.  True simultaneous hole punching is planned for v3 with an
//! out-of-band signalling channel.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use bytecodec::DecodeExt;
use stun_codec::rfc5389::attributes::XorMappedAddress;
use stun_codec::rfc5389::methods::BINDING;
use stun_codec::rfc5389::Attribute;
use stun_codec::{Message, MessageClass, MessageDecoder, MessageEncoder, TransactionId};
use tokio::net::UdpSocket;
use tokio::time::timeout;

use bytecodec::EncodeExt;

use crate::p2p::{P2pError, Result};

// ── STUN servers ──────────────────────────────────────────────────────────────

const STUN_PRIMARY: &str = "stun.l.google.com:19302";
const STUN_FALLBACK: &str = "stun.cloudflare.com:3478";
const STUN_TIMEOUT: Duration = Duration::from_secs(5);

// ── connect timeout (WAN TCP) ─────────────────────────────────────────────────

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

// ── Public API ────────────────────────────────────────────────────────────────

/// Discover the public `SocketAddr` that remote peers should use to reach this
/// node's TCP server listening on `tcp_port`.
///
/// Internally binds a **separate, ephemeral UDP socket** (port 0) to query
/// STUN — this avoids any conflict with the `TcpListener` already bound on
/// `tcp_port`.  The returned address combines:
/// - **IP** as seen by the STUN server (the public WAN IP of the seeder)
/// - **Port** = `tcp_port` (the TCP listener port already open)
///
/// Falls back from `stun.l.google.com:19302` to `stun.cloudflare.com:3478` on
/// any error.  If both fail, returns `Err(P2pError::Nat(...))`.
pub async fn discover_public_addr(tcp_port: u16) -> Result<SocketAddr> {
    match discover_public_addr_with_server(tcp_port, STUN_PRIMARY).await {
        Ok(addr) => {
            eprintln!("[nat] public addr via STUN ({STUN_PRIMARY}): {addr}");
            Ok(addr)
        }
        Err(e) => {
            eprintln!("[nat] primary STUN failed ({e}), trying fallback {STUN_FALLBACK}");
            let addr = discover_public_addr_with_server(tcp_port, STUN_FALLBACK).await?;
            eprintln!("[nat] public addr via STUN ({STUN_FALLBACK}): {addr}");
            Ok(addr)
        }
    }
}

/// Same as [`discover_public_addr`] but with an injectable STUN server address.
/// Declared `pub(crate)` so tests can point it at a mock server on loopback.
pub(crate) async fn discover_public_addr_with_server(
    tcp_port: u16,
    stun_server: &str,
) -> Result<SocketAddr> {
    // Resolve the STUN server address (DNS lookup happens here)
    let stun_addr: SocketAddr = tokio::net::lookup_host(stun_server)
        .await
        .map_err(P2pError::Io)?
        .next()
        .ok_or_else(|| P2pError::Nat(format!("could not resolve STUN server: {stun_server}")))?;

    // Bind an ephemeral UDP socket — port 0, OS chooses.
    // This never conflicts with the TCP listener.
    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(P2pError::Io)?;
    socket.connect(stun_addr).await.map_err(P2pError::Io)?;

    // ── Build STUN Binding Request ────────────────────────────────────────────
    let transaction_id = TransactionId::new(rand_transaction_id());
    let request: Message<Attribute> = Message::new(MessageClass::Request, BINDING, transaction_id);

    let mut encoder = MessageEncoder::new();
    let bytes = encoder
        .encode_into_bytes(request)
        .map_err(|e| P2pError::Nat(format!("STUN encode: {e}")))?;

    // ── Send and receive with timeout ─────────────────────────────────────────
    let public_ip: IpAddr = timeout(STUN_TIMEOUT, async {
        socket.send(&bytes).await.map_err(P2pError::Io)?;

        let mut buf = [0u8; 1024];
        let n = socket.recv(&mut buf).await.map_err(P2pError::Io)?;

        // Decode STUN response
        let mut decoder = MessageDecoder::<Attribute>::new();
        let response = decoder
            .decode_from_bytes(&buf[..n])
            .map_err(|e| P2pError::Nat(format!("STUN decode: {e}")))?
            .map_err(|e| P2pError::Nat(format!("STUN incomplete: {e:?}")))?;

        if response.class() != MessageClass::SuccessResponse {
            return Err(P2pError::Nat(format!(
                "STUN error response: {:?}",
                response.class()
            )));
        }

        // Extract XOR-MAPPED-ADDRESS
        let mapped = response
            .get_attribute::<XorMappedAddress>()
            .ok_or_else(|| P2pError::Nat("STUN response missing XOR-MAPPED-ADDRESS".into()))?;

        Ok(mapped.address().ip())
    })
    .await
    .map_err(|_| P2pError::Nat(format!("STUN timeout after {STUN_TIMEOUT:?}")))??;

    // Combine the WAN IP with the TCP listener port
    Ok(SocketAddr::new(public_ip, tcp_port))
}

/// Attempt a TCP connection to a remote WAN peer with a 10-second timeout.
///
/// Returns a [`tokio::net::TcpStream`] ready for the Noise XX handshake.
///
/// # NAT compatibility
/// | NAT type | Result |
/// |---|---|
/// | Open / Full-cone | ✅ Connects directly |
/// | Restricted-cone / Port-restricted cone | ✅ Connects if seeder listener is active |
/// | Symmetric / CGNAT | ❌ Returns `Err` with user-friendly message |
pub async fn try_connect_wan(peer_addr: SocketAddr) -> Result<tokio::net::TcpStream> {
    timeout(CONNECT_TIMEOUT, tokio::net::TcpStream::connect(peer_addr))
        .await
        .map_err(|_| {
            P2pError::Nat(format!(
                "TCP connection timeout after {CONNECT_TIMEOUT:?} to {peer_addr}. \
                Si el seeder está detrás de NAT simétrico o CGNAT, \
                requiere port forwarding manual en el puerto {}.",
                peer_addr.port()
            ))
        })?
        .map_err(|e| {
            P2pError::Nat(format!(
                "No se pudo conectar a {peer_addr}: {e}. \
                Si el seeder está detrás de NAT simétrico o CGNAT, \
                requiere port forwarding manual en el puerto {}.",
                peer_addr.port()
            ))
        })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Generate a random 12-byte STUN transaction ID without pulling in `rand`.
fn rand_transaction_id() -> [u8; 12] {
    // Use the current time and stack address entropy — sufficient for a
    // one-shot STUN request (not a security-sensitive nonce).
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let p = &t as *const u32 as usize; // stack address entropy
    let mut id = [0u8; 12];
    for (i, b) in id.iter_mut().enumerate() {
        *b = ((t >> (i % 4 * 8)) ^ (p >> (i % 8)) as u32) as u8;
    }
    id
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    use bytecodec::DecodeExt;
    use bytecodec::EncodeExt;
    use stun_codec::rfc5389::attributes::XorMappedAddress;
    use stun_codec::rfc5389::Attribute;
    use stun_codec::{Message, MessageClass, MessageDecoder, MessageEncoder};

    // ── 1. STUN discovery with mock server ───────────────────────────────────

    /// Spawns a mock STUN server on loopback that returns a fixed
    /// XOR-MAPPED-ADDRESS of 1.2.3.4.  Verifies that `discover_public_addr_with_server`
    /// correctly extracts the IP and combines it with `tcp_port`.
    #[tokio::test]
    async fn stun_discovery_returns_public_addr() {
        let fake_ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        const TCP_PORT: u16 = 8765;

        // Bind mock server
        let mock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mock_addr = mock.local_addr().unwrap();
        let mock_server_str = mock_addr.to_string();

        // Spawn mock server task
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let (n, client_addr) = mock.recv_from(&mut buf).await.unwrap();

            // Decode the request to get transaction_id
            let mut decoder = MessageDecoder::<Attribute>::new();
            let request = decoder.decode_from_bytes(&buf[..n]).unwrap().unwrap();

            // Build success response with XOR-MAPPED-ADDRESS
            // stun_codec 0.3: XorMappedAddress::new takes only SocketAddr
            let xor_addr = XorMappedAddress::new(SocketAddr::new(fake_ip, TCP_PORT));
            let mut response: Message<Attribute> = Message::new(
                MessageClass::SuccessResponse,
                BINDING,
                request.transaction_id(),
            );
            response.add_attribute(Attribute::XorMappedAddress(xor_addr));

            let mut encoder = MessageEncoder::new();
            let resp_bytes = encoder.encode_into_bytes(response).unwrap();
            mock.send_to(&resp_bytes, client_addr).await.unwrap();
        });

        let result = discover_public_addr_with_server(TCP_PORT, &mock_server_str)
            .await
            .expect("STUN discovery failed");

        assert_eq!(
            result.ip(),
            fake_ip,
            "IP should match mock XOR-MAPPED-ADDRESS"
        );
        assert_eq!(
            result.port(),
            TCP_PORT,
            "Port should be the tcp_port, not the STUN port"
        );
    }

    // ── 2. WAN connect loopback ───────────────────────────────────────────────

    /// Verifies that `try_connect_wan` can establish a real TCP connection
    /// on loopback and that the resulting stream can do a round-trip of 4 bytes.
    #[tokio::test]
    async fn wan_connect_loopback() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Accept exactly one connection
        let accept_task = tokio::spawn(async move {
            let (mut conn, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4];
            tokio::io::AsyncReadExt::read_exact(&mut conn, &mut buf)
                .await
                .unwrap();
            tokio::io::AsyncWriteExt::write_all(&mut conn, &buf)
                .await
                .unwrap();
        });

        let mut stream = try_connect_wan(server_addr)
            .await
            .expect("try_connect_wan failed on loopback");

        // Round-trip 4 bytes
        tokio::io::AsyncWriteExt::write_all(&mut stream, b"PING")
            .await
            .unwrap();
        let mut pong = [0u8; 4];
        tokio::io::AsyncReadExt::read_exact(&mut stream, &mut pong)
            .await
            .unwrap();

        assert_eq!(&pong, b"PING");
        let _ = accept_task.await;
    }

    // ── 3. WAN connect fails gracefully ───────────────────────────────────────

    /// Verifies that `try_connect_wan` returns a descriptive `Err` on an
    /// unreachable address — no panic, no hang (uses port 1 which is closed).
    #[tokio::test]
    async fn wan_connect_fails_gracefully() {
        // Port 1 is virtually never open; connection should be refused quickly.
        let closed: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let result = try_connect_wan(closed).await;

        assert!(result.is_err(), "should fail for closed port");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("No se pudo conectar") || msg.contains("timeout"),
            "error message should be descriptive, got: {msg}"
        );
    }
}
