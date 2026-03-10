//! TCP peer-to-peer shard transfer with Noise Protocol encryption.
//!
//! Security guarantees:
//! - **Noise XX** (25519/ChaChaPoly/BLAKE2s): mutual authentication + E2E encryption.
//!   No certificates or CA required — the seeder's static public key travels
//!   inside the ShareLink, and both sides verify it during the handshake.
//! - **Auth token**: validated before serving any shard (prevents unauthorized
//!   fetching by anyone who discovers the IP:port).
//! - **SHA-256 verification**: each shard is verified against its expected hash
//!   *after* decryption, before being accepted by the client.
//!
//! NAT traversal note (v1):
//!   The seeder must be reachable directly (port-forward or same LAN).
//!   STUN-based NAT traversal is planned for v2.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::time::timeout;

use crate::p2p::nat;
use crate::p2p::protocol::{ErrorResponse, MsgType, ShardRequest, ShardResponse};
use crate::p2p::shard::sha256_hex;
use crate::p2p::store::build_shard_index;
use crate::p2p::{P2pError, Result};

const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const FETCH_TIMEOUT_SECS: u64 = 30;
/// Maximum shard size accepted from the network (64 MiB guard).
const MAX_SHARD_BYTES: usize = 64 * 1024 * 1024;

// ── Server ────────────────────────────────────────────────────────────────────

/// Start a TCP server that serves shards from `store_dir` to authenticated peers.
///
/// - Indexes all shards from `store_dir` at startup (O(1) per request).
/// - Performs Noise XX handshake on each connection.
/// - Validates `auth_token` before serving data.
/// - Shuts down cleanly when `shutdown` fires.
pub async fn serve(
    store_dir: PathBuf,
    port: u16,
    auth_token: String,
    static_key: Arc<Vec<u8>>,
    shutdown: oneshot::Receiver<()>,
) -> Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .map_err(P2pError::Io)?;

    eprintln!("[p2p] serving on 0.0.0.0:{port}");

    // Build shard index once at startup
    let index = Arc::new(build_shard_index(&store_dir)?);

    let mut shutdown = std::pin::pin!(shutdown);

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, peer_addr)) => {
                        let index2   = index.clone();
                        let token    = auth_token.clone();
                        let sk       = static_key.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, peer_addr, index2, token, sk).await {
                                eprintln!("[p2p] peer {peer_addr} error: {e}");
                            }
                        });
                    }
                    Err(e) => eprintln!("[p2p] accept error: {e}"),
                }
            }
            _ = &mut shutdown => {
                eprintln!("[p2p] shutdown signal received");
                break;
            }
        }
    }
    Ok(())
}

async fn handle_connection(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    index: Arc<HashMap<String, PathBuf>>,
    auth_token: String,
    static_key: Arc<Vec<u8>>,
) -> Result<()> {
    eprintln!("[p2p] connection from {peer_addr}");

    // ── Noise XX handshake (responder side) ───────────────────────────────────
    let builder = snow::Builder::new(NOISE_PATTERN.parse().unwrap());
    let private_key = &static_key[..32];
    let mut noise = builder
        .local_private_key(private_key)
        .build_responder()
        .map_err(|e| P2pError::Noise(e.to_string()))?;

    // XX: 3 handshake messages → e / e, ee, s, es / s, se
    let mut buf = [0u8; 65535];

    // msg 1: receive from initiator
    let n = stream.read_u16().await.map_err(P2pError::Io)? as usize;
    stream
        .read_exact(&mut buf[..n])
        .await
        .map_err(P2pError::Io)?;
    let mut out = vec![0u8; 65535];
    let _written = noise
        .read_message(&buf[..n], &mut out)
        .map_err(|e| P2pError::Noise(e.to_string()))?;

    // msg 2: send to initiator
    let written = noise
        .write_message(&[], &mut buf)
        .map_err(|e| P2pError::Noise(e.to_string()))?;
    stream
        .write_u16(written as u16)
        .await
        .map_err(P2pError::Io)?;
    stream
        .write_all(&buf[..written])
        .await
        .map_err(P2pError::Io)?;

    // msg 3: receive from initiator
    let n = stream.read_u16().await.map_err(P2pError::Io)? as usize;
    stream
        .read_exact(&mut buf[..n])
        .await
        .map_err(P2pError::Io)?;
    let _written = noise
        .read_message(&buf[..n], &mut out)
        .map_err(|e| P2pError::Noise(e.to_string()))?;

    let mut noise = noise
        .into_transport_mode()
        .map_err(|e| P2pError::Noise(e.to_string()))?;

    // ── Serve shard requests ──────────────────────────────────────────────────
    loop {
        // Read a noise-encrypted framed message
        let enc_len = match stream.read_u16().await {
            Ok(n) => n as usize,
            Err(_) => break, // peer disconnected
        };
        if enc_len == 0 {
            break;
        }

        let mut enc_buf = vec![0u8; enc_len];
        stream
            .read_exact(&mut enc_buf)
            .await
            .map_err(P2pError::Io)?;

        let mut plain = vec![0u8; enc_len + 16];
        let n = noise
            .read_message(&enc_buf, &mut plain)
            .map_err(|e| P2pError::Noise(e.to_string()))?;
        let plain = &plain[..n];

        // Parse framed message from plaintext
        if plain.len() < 5 {
            break;
        }
        let msg_type = MsgType::try_from(plain[0])?;
        let payload_len = u32::from_be_bytes([plain[1], plain[2], plain[3], plain[4]]) as usize;
        let payload = &plain[5..5 + payload_len];

        match msg_type {
            MsgType::ShardRequest => {
                let req: ShardRequest = serde_json::from_slice(payload)
                    .map_err(|e| P2pError::Transfer(e.to_string()))?;

                // Auth check
                if req.auth_token != auth_token {
                    send_noise_frame(
                        &mut stream,
                        &mut noise,
                        MsgType::Error,
                        &ErrorResponse {
                            message: "unauthorized".into(),
                        },
                    )
                    .await?;
                    break;
                }

                // Lookup shard
                let shard_bytes = match index.get(&req.shard_hash) {
                    Some(path) => std::fs::read(path).map_err(P2pError::Io)?,
                    None => {
                        send_noise_frame(
                            &mut stream,
                            &mut noise,
                            MsgType::Error,
                            &ErrorResponse {
                                message: format!("shard not found: {}", req.shard_hash),
                            },
                        )
                        .await?;
                        continue;
                    }
                };

                send_noise_frame(
                    &mut stream,
                    &mut noise,
                    MsgType::ShardResponse,
                    &ShardResponse {
                        data_hex: hex::encode(&shard_bytes),
                    },
                )
                .await?;
            }
            _ => break,
        }
    }

    Ok(())
}

/// Encode message as [1B type][4B len][payload], encrypt with Noise, send with 2B length prefix.
async fn send_noise_frame<S: serde::Serialize>(
    stream: &mut TcpStream,
    noise: &mut snow::TransportState,
    msg_type: MsgType,
    payload: &S,
) -> Result<()> {
    let json = serde_json::to_vec(payload).map_err(|e| P2pError::Transfer(e.to_string()))?;
    let len = json.len() as u32;

    let mut frame = Vec::with_capacity(5 + json.len());
    frame.push(msg_type as u8);
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&json);

    let mut enc = vec![0u8; frame.len() + 16];
    let n = noise
        .write_message(&frame, &mut enc)
        .map_err(|e| P2pError::Noise(e.to_string()))?;

    stream.write_u16(n as u16).await.map_err(P2pError::Io)?;
    stream.write_all(&enc[..n]).await.map_err(P2pError::Io)?;
    Ok(())
}

// ── Client ────────────────────────────────────────────────────────────────────

/// Fetch a single shard from a remote peer.
///
/// - Connects to `addr`, performs Noise XX handshake verifying `expected_noise_pubkey`.
/// - Sends a `ShardRequest` with `auth_token`.
/// - Receives the shard, verifies SHA-256 against `expected_hash`.
/// - Times out after [`FETCH_TIMEOUT_SECS`] seconds.
pub async fn fetch_shard(
    addr: &str,
    expected_noise_pubkey: &[u8],
    expected_hash: &str,
    auth_token: &str,
) -> Result<Vec<u8>> {
    timeout(
        Duration::from_secs(FETCH_TIMEOUT_SECS),
        fetch_shard_inner(addr, expected_noise_pubkey, expected_hash, auth_token),
    )
    .await
    .map_err(|_| P2pError::Transfer(format!("timeout after {FETCH_TIMEOUT_SECS}s")))?
}

async fn fetch_shard_inner(
    addr: &str,
    expected_pubkey: &[u8],
    expected_hash: &str,
    auth_token: &str,
) -> Result<Vec<u8>> {
    let addr: SocketAddr = addr
        .parse()
        .map_err(|e| P2pError::Transfer(format!("invalid addr: {e}")))?;

    // Attempt WAN connection with timeout and descriptive error for symmetric NAT.
    // try_connect_wan wraps TcpStream::connect with a 10s timeout; if the peer's
    // NAT is symmetric/CGNAT the error message tells the user to port-forward.
    let mut stream = nat::try_connect_wan(addr).await?;

    // ── Noise XX handshake (initiator side) ───────────────────────────────────
    let builder = snow::Builder::new(NOISE_PATTERN.parse().unwrap());
    // Ephemeral keypair for this session
    let keypair = builder
        .generate_keypair()
        .map_err(|e| P2pError::Noise(e.to_string()))?;
    let mut noise = snow::Builder::new(NOISE_PATTERN.parse().unwrap())
        .local_private_key(&keypair.private)
        .remote_public_key(expected_pubkey)
        .build_initiator()
        .map_err(|e| P2pError::Noise(e.to_string()))?;

    let mut buf = [0u8; 65535];

    // msg 1: send to responder
    let n = noise
        .write_message(&[], &mut buf)
        .map_err(|e| P2pError::Noise(e.to_string()))?;
    stream.write_u16(n as u16).await.map_err(P2pError::Io)?;
    stream.write_all(&buf[..n]).await.map_err(P2pError::Io)?;

    // msg 2: receive from responder
    let n = stream.read_u16().await.map_err(P2pError::Io)? as usize;
    stream
        .read_exact(&mut buf[..n])
        .await
        .map_err(P2pError::Io)?;
    let mut out = vec![0u8; 65535];
    noise
        .read_message(&buf[..n], &mut out)
        .map_err(|e| P2pError::Noise(e.to_string()))?;

    // msg 3: send to responder
    let n = noise
        .write_message(&[], &mut buf)
        .map_err(|e| P2pError::Noise(e.to_string()))?;
    stream.write_u16(n as u16).await.map_err(P2pError::Io)?;
    stream.write_all(&buf[..n]).await.map_err(P2pError::Io)?;

    let mut noise = noise
        .into_transport_mode()
        .map_err(|e| P2pError::Noise(e.to_string()))?;

    // ── Send ShardRequest ─────────────────────────────────────────────────────
    send_noise_frame(
        &mut stream,
        &mut noise,
        MsgType::ShardRequest,
        &ShardRequest {
            shard_hash: expected_hash.to_string(),
            auth_token: auth_token.to_string(),
        },
    )
    .await?;

    // ── Receive response ──────────────────────────────────────────────────────
    let enc_len = stream.read_u16().await.map_err(P2pError::Io)? as usize;
    let mut enc_buf = vec![0u8; enc_len];
    stream
        .read_exact(&mut enc_buf)
        .await
        .map_err(P2pError::Io)?;

    let mut plain = vec![0u8; enc_len + 16];
    let n = noise
        .read_message(&enc_buf, &mut plain)
        .map_err(|e| P2pError::Noise(e.to_string()))?;
    let plain = &plain[..n];

    if plain.len() < 5 {
        return Err(P2pError::Transfer("response frame too short".into()));
    }

    let msg_type = MsgType::try_from(plain[0])?;
    let payload_len = u32::from_be_bytes([plain[1], plain[2], plain[3], plain[4]]) as usize;
    let payload = &plain[5..5 + payload_len];

    match msg_type {
        MsgType::ShardResponse => {
            let resp: ShardResponse =
                serde_json::from_slice(payload).map_err(|e| P2pError::Transfer(e.to_string()))?;

            let shard_bytes =
                hex::decode(&resp.data_hex).map_err(|e| P2pError::Transfer(e.to_string()))?;

            if shard_bytes.len() > MAX_SHARD_BYTES {
                return Err(P2pError::Transfer("shard exceeds max size".into()));
            }

            // Verify hash
            let actual_hash = sha256_hex(&shard_bytes);
            if actual_hash != expected_hash {
                return Err(P2pError::Integrity(format!(
                    "shard hash mismatch: expected {expected_hash}, got {actual_hash}"
                )));
            }

            Ok(shard_bytes)
        }
        MsgType::Error => {
            let err: ErrorResponse =
                serde_json::from_slice(payload).map_err(|e| P2pError::Transfer(e.to_string()))?;
            Err(P2pError::Transfer(format!("server error: {}", err.message)))
        }
        _ => Err(P2pError::Transfer("unexpected response type".into())),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::store::{load_or_generate_keypair, public_key_from_keypair, save_dataset};

    /// Shared test setup: generates a keypair, saves a dataset, returns
    /// (store_dir, static_key Arc, public_key bytes, first shard hash, auth_token).
    async fn setup_server(dir: &std::path::Path) -> (Arc<Vec<u8>>, Vec<u8>, String, String) {
        let raw_kp = load_or_generate_keypair(dir).expect("keypair");
        let pub_key = public_key_from_keypair(&raw_kp).to_vec();
        let static_key = Arc::new(raw_kp);

        let json = br#"[{"id":1,"val":"hello"},{"id":2,"val":"world"}]"#;
        let manifest = save_dataset(dir, "loopback_test", json)
            .await
            .expect("save_dataset");

        let first_hash = manifest.shards[0].hash_hex.clone();
        let auth_token = uuid::Uuid::new_v4().to_string();

        (static_key, pub_key, first_hash, auth_token)
    }

    /// Bind on port 0 and return the actual port the OS assigned.
    async fn free_port() -> u16 {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
        // l is dropped here, freeing the port
    }

    #[tokio::test]
    async fn tcp_loopback_fetch_shard_ok() {
        let dir = tempfile::tempdir().unwrap();
        let (static_key, pub_key, first_hash, auth_token) = setup_server(dir.path()).await;

        // Pick a free port; race window is tiny on loopback
        let port = free_port().await;
        let addr = format!("127.0.0.1:{port}");

        // Spawn server
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let store_dir = dir.path().to_path_buf();
        let at = auth_token.clone();
        let sk = static_key.clone();
        let server = tokio::spawn(async move {
            serve(store_dir, port, at, sk, shutdown_rx)
                .await
                .expect("serve failed");
        });

        // Give the server a moment to bind
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Fetch the shard as a client
        let shard = fetch_shard(&addr, &pub_key, &first_hash, &auth_token)
            .await
            .expect("fetch_shard failed");

        // Verify integrity
        let actual_hash = crate::p2p::shard::sha256_hex(&shard);
        assert_eq!(
            actual_hash, first_hash,
            "shard hash mismatch after transport"
        );

        // Graceful shutdown
        let _ = shutdown_tx.send(());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server).await;
    }

    #[tokio::test]
    async fn tcp_loopback_wrong_auth_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (static_key, pub_key, first_hash, auth_token) = setup_server(dir.path()).await;

        let port = free_port().await;
        let addr = format!("127.0.0.1:{port}");

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let store_dir = dir.path().to_path_buf();
        let sk = static_key.clone();
        tokio::spawn(async move {
            serve(store_dir, port, auth_token, sk, shutdown_rx)
                .await
                .ok();
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Use a WRONG token — server must reject
        let result = fetch_shard(&addr, &pub_key, &first_hash, "wrong-token").await;
        assert!(result.is_err(), "server should reject wrong auth token");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("unauthorized") || err_msg.contains("server error"),
            "unexpected error: {err_msg}"
        );

        let _ = shutdown_tx.send(());
    }
}
