// The p2p module is fully implemented but not yet wired to the UI.
// All items will be used in a future integration step.
#![allow(dead_code, unused_imports)]

pub mod compress;
pub mod discovery;
pub mod nat;
pub mod protocol;
pub mod shard;
pub mod store;
pub mod transfer;

use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine as _;
use discovery::ShareLink;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum P2pError {
    #[error("compression error: {0}")]
    Compress(String),
    #[error("shard integrity error: {0}")]
    Integrity(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("discovery error: {0}")]
    Discovery(String),
    #[error("transfer error: {0}")]
    Transfer(String),
    #[error("noise protocol error: {0}")]
    Noise(String),
    #[error("NAT traversal error: {0}")]
    Nat(String),
}

pub type Result<T> = std::result::Result<T, P2pError>;

// ── Commands (UI → async runtime) ────────────────────────────────────────────

/// Commands sent from the egui UI to the async P2P runtime.
///
/// **Usage from egui (sync context):**
/// ```
/// let _ = cmd_tx.try_send(Command::Shutdown);
/// ```
/// Each variant that expects a response includes a `resp` oneshot sender so
/// the UI can poll the receiver on the next frame.
pub enum Command {
    /// Compress, shard and save a dataset, then start serving it on `port`.
    /// `resp` receives the full "exphora:..." share-link string.
    ShareDataset {
        name: String,
        json_bytes: Vec<u8>,
        port: u16,
        resp: oneshot::Sender<std::result::Result<String, String>>,
    },
    /// Fetch a remote dataset described by a share link.
    /// `resp` receives the decompressed JSON bytes, ready to parse.
    FetchDataset {
        link: String,
        resp: oneshot::Sender<std::result::Result<Vec<u8>, String>>,
    },
    /// Probe the NAT / STUN to discover the public IP:port for the given local port.
    /// `resp` receives Ok(SocketAddr) on success or Err(String) on failure.
    DetectNat {
        port: u16,
        resp: oneshot::Sender<std::result::Result<std::net::SocketAddr, String>>,
    },
    /// Gracefully stop the async runtime.
    Shutdown,
}

// ── Event loop ────────────────────────────────────────────────────────────────

/// Main async loop run in a dedicated OS thread.
///
/// ```rust
/// // In main.rs (before eframe::run_native):
/// let rt = tokio::runtime::Runtime::new().unwrap();
/// let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<p2p::Command>(32);
/// std::thread::spawn(move || rt.block_on(p2p::run_event_loop(cmd_rx)));
/// ```
pub async fn run_event_loop(mut rx: mpsc::Receiver<Command>) {
    // Resolve store directory: <data_dir>/exphora_p2p
    let store_dir: PathBuf = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("exphora_p2p");

    std::fs::create_dir_all(&store_dir).ok();

    // Load or generate the persistent Noise static keypair once.
    let static_key = match store::load_or_generate_keypair(&store_dir) {
        Ok(k) => Arc::new(k),
        Err(e) => {
            eprintln!("[p2p] keypair error: {e}");
            return;
        }
    };

    while let Some(cmd) = rx.recv().await {
        match cmd {
            Command::ShareDataset {
                name,
                json_bytes,
                port,
                resp,
            } => {
                let store_dir2 = store_dir.clone();
                let sk = static_key.clone();
                tokio::spawn(async move {
                    let result = handle_share(store_dir2, name, json_bytes, port, sk).await;
                    let _ = resp.send(result.map_err(|e| e.to_string()));
                });
            }
            Command::FetchDataset { link, resp } => {
                tokio::spawn(async move {
                    let result = handle_fetch(link).await;
                    let _ = resp.send(result.map_err(|e| e.to_string()));
                });
            }
            Command::DetectNat { port, resp } => {
                tokio::spawn(async move {
                    let result = nat::discover_public_addr(port)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = resp.send(result);
                });
            }
            Command::Shutdown => {
                break;
            }
        }
    }
}

// ── Command handlers ──────────────────────────────────────────────────────────

async fn handle_share(
    store_dir: PathBuf,
    name: String,
    json_bytes: Vec<u8>,
    port: u16,
    static_key: Arc<Vec<u8>>,
) -> Result<String> {
    // 1. Compress + shard + save to disk
    let manifest = store::save_dataset(&store_dir, &name, &json_bytes).await?;

    // 2. Derive the public key (last 32 bytes of the keypair stored as [priv‖pub])
    let noise_pubkey_b64 = {
        let pub_bytes = &static_key[static_key.len() - 32..];
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pub_bytes)
    };

    // 3. Generate auth token
    let auth_token = uuid::Uuid::new_v4().to_string();

    // 4. Spawn TCP server FIRST — the NAT must see the port open before STUN
    //    queries so that some NATs correctly report the mapped port.
    let (_, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let store2 = store_dir.clone();
    let at = auth_token.clone();
    let sk = static_key.clone();
    tokio::spawn(async move {
        if let Err(e) = transfer::serve(store2, port, at, sk, shutdown_rx).await {
            eprintln!("[p2p] serve error: {e}");
        }
    });

    // 5. Discover public WAN address via STUN (UDP ephemeral, no port conflict).
    //    Always include the local fallback address so LAN peers can still connect.
    let local_addr = format!("0.0.0.0:{port}");
    let mut addrs = vec![local_addr];
    match nat::discover_public_addr(port).await {
        Ok(public_addr) => {
            // Insert WAN address first so fetchers try it before the LAN addr.
            addrs.insert(0, public_addr.to_string());
        }
        Err(e) => {
            eprintln!("[p2p] STUN discovery failed ({e}); ShareLink will use local addr only");
        }
    }

    // 6. Build share link string ("exphora:...")
    let link = discovery::generate_link(&manifest, &auth_token, &addrs, &noise_pubkey_b64);

    // Return the link string directly — caller displays it; no ShareLink::Display needed
    Ok(link)
}

async fn handle_fetch(link: String) -> Result<Vec<u8>> {
    let share = discovery::parse_link(&link)?;

    // Decode the seeder Noise static public key
    let noise_pubkey = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&share.seeder_noise_pubkey)
        .map_err(|e| P2pError::Discovery(e.to_string()))?;

    let addr = share
        .seeder_addrs
        .first()
        .ok_or_else(|| P2pError::Discovery("no seeders in link".into()))?;

    // Fetch each shard and verify its SHA-256 hash before accepting it.
    // Note: `manifest_hash` in the ShareLink is a hash of the *full* ShardManifest
    // (including fields like `id`, `name`, `original_size` that the receiver does
    // not have). Per-shard verification against `shard_hashes` gives integrity
    // guarantees equivalent to what the receiver can achieve with available data.
    let mut raw_shards: Vec<Vec<u8>> = Vec::new();
    for (i, expected_hash) in share.shard_hashes.iter().enumerate() {
        let shard =
            transfer::fetch_shard(addr, &noise_pubkey, expected_hash, &share.auth_token).await?;
        let actual_hash = shard::sha256_hex(&shard);
        if actual_hash != *expected_hash {
            return Err(P2pError::Integrity(format!(
                "shard {i}: expected hash {expected_hash}, got {actual_hash}"
            )));
        }
        raw_shards.push(shard);
    }

    // Concatenate verified shards → compressed payload → decompressed JSON.
    // compress::decompress uses spawn_blocking internally; safe in async context.
    let combined: Vec<u8> = raw_shards.into_iter().flatten().collect();
    let json_bytes = compress::decompress(combined).await?;
    Ok(json_bytes)
}
