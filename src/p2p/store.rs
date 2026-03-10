//! Persistent storage for P2P datasets and the Noise static keypair.
//!
//! Layout on disk:
//! ```text
//! <store_dir>/
//! ├── noise_static.key          ← 64-byte keypair [private‖public]
//! └── datasets/
//!     └── <manifest_id>/
//!         ├── manifest.json
//!         └── 0.shard, 1.shard, ...
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::p2p::compress::decompress_sync;
use crate::p2p::shard::{reconstruct, split, ShardManifest, DEFAULT_SHARD_SIZE};
use crate::p2p::{P2pError, Result};

// ── Noise keypair ─────────────────────────────────────────────────────────────

/// Load the persistent 64-byte Noise static keypair (first 32 = private, last 32 = public).
/// If the file does not exist, a new keypair is generated and saved.
pub fn load_or_generate_keypair(store_dir: &Path) -> Result<Vec<u8>> {
    let key_path = store_dir.join("noise_static.key");
    if key_path.exists() {
        let bytes = std::fs::read(&key_path)?;
        if bytes.len() == 64 {
            return Ok(bytes);
        }
        // Corrupted — regenerate
        eprintln!("[p2p] noise_static.key corrupted, regenerating");
    }

    // Generate a new Noise XX static keypair using snow
    let builder = snow::Builder::new("Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap());
    let keypair = builder
        .generate_keypair()
        .map_err(|e| P2pError::Noise(e.to_string()))?;

    // Store as [private(32) ‖ public(32)]
    let mut raw = Vec::with_capacity(64);
    raw.extend_from_slice(&keypair.private);
    raw.extend_from_slice(&keypair.public);

    std::fs::write(&key_path, &raw)?;
    eprintln!(
        "[p2p] Generated new Noise static keypair → {}",
        key_path.display()
    );
    Ok(raw)
}

/// Extract the public key bytes from a stored 64-byte keypair.
pub fn public_key_from_keypair(keypair: &[u8]) -> &[u8] {
    &keypair[32..]
}

// ── Dataset storage ───────────────────────────────────────────────────────────

fn dataset_dir(store_dir: &Path, manifest_id: &str) -> PathBuf {
    store_dir.join("datasets").join(manifest_id)
}

/// Compress `json_bytes`, split into shards, and persist everything under `store_dir`.
/// Returns the manifest (which the caller uses to build a ShareLink).
pub async fn save_dataset(
    store_dir: &Path,
    name: &str,
    json_bytes: &[u8],
) -> Result<ShardManifest> {
    let compressed = crate::p2p::compress::compress(json_bytes.to_vec()).await?;
    let id = uuid::Uuid::new_v4().to_string();

    let (manifest, shards) = split(&id, name, json_bytes.len(), &compressed, DEFAULT_SHARD_SIZE);

    let dir = dataset_dir(store_dir, &id);
    std::fs::create_dir_all(&dir)?;

    // Write each shard
    for (meta, shard) in manifest.shards.iter().zip(shards.iter()) {
        let shard_path = dir.join(format!("{}.shard", meta.index));
        std::fs::write(shard_path, shard)?;
    }

    // Write manifest
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| P2pError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    std::fs::write(dir.join("manifest.json"), manifest_json)?;

    Ok(manifest)
}

/// Load a dataset from disk given its manifest path.
/// Reads all shards, verifies hashes, reconstructs and decompresses.
pub fn load_dataset(manifest_path: &Path) -> Result<Vec<u8>> {
    let manifest_json = std::fs::read_to_string(manifest_path)?;
    let manifest: ShardManifest = serde_json::from_str(&manifest_json)
        .map_err(|e| P2pError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    let dir = manifest_path
        .parent()
        .ok_or_else(|| P2pError::Io(std::io::Error::new(std::io::ErrorKind::Other, "bad path")))?;

    let mut shards: Vec<Vec<u8>> = Vec::with_capacity(manifest.total_shards as usize);
    for meta in &manifest.shards {
        let shard_path = dir.join(format!("{}.shard", meta.index));
        let shard = std::fs::read(shard_path)?;
        shards.push(shard);
    }

    let compressed = reconstruct(&manifest, &shards)?;
    let decompressed = decompress_sync(&compressed)?;
    Ok(decompressed)
}

// ── Shard index (used by the TCP server) ─────────────────────────────────────

/// Walk `store_dir/datasets/` and build a flat map: shard_hash → shard_file_path.
/// The server uses this for O(1) lookup without scanning disk per request.
pub fn build_shard_index(store_dir: &Path) -> Result<HashMap<String, PathBuf>> {
    let mut index = HashMap::new();
    let datasets_dir = store_dir.join("datasets");

    if !datasets_dir.exists() {
        return Ok(index);
    }

    for entry in std::fs::read_dir(&datasets_dir)? {
        let entry = entry?;
        let manifest_path = entry.path().join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }

        let manifest_json = std::fs::read_to_string(&manifest_path)?;
        let manifest: ShardManifest = match serde_json::from_str(&manifest_json) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[p2p] skipping corrupt manifest {:?}: {e}", manifest_path);
                continue;
            }
        };

        for meta in &manifest.shards {
            let shard_path = entry.path().join(format!("{}.shard", meta.index));
            if shard_path.exists() {
                index.insert(meta.hash_hex.clone(), shard_path);
            }
        }
    }

    Ok(index)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn round_trip_disk() {
        let dir = tempfile::tempdir().unwrap();
        let json = br#"[{"x":1},{"x":2},{"x":3}]"#;

        let manifest = save_dataset(dir.path(), "test", json)
            .await
            .expect("save failed");
        let manifest_path = dir
            .path()
            .join("datasets")
            .join(&manifest.id)
            .join("manifest.json");
        let restored = load_dataset(&manifest_path).expect("load failed");

        assert_eq!(restored, json.to_vec());
    }

    #[tokio::test]
    async fn shard_index_populated() {
        let dir = tempfile::tempdir().unwrap();
        let json = b"[1, 2, 3]";
        let manifest = save_dataset(dir.path(), "idx_test", json)
            .await
            .expect("save");
        let index = build_shard_index(dir.path()).expect("index");

        for meta in &manifest.shards {
            assert!(
                index.contains_key(&meta.hash_hex),
                "index missing shard {}",
                meta.hash_hex
            );
        }
    }

    #[test]
    fn keypair_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let kp1 = load_or_generate_keypair(dir.path()).expect("gen");
        let kp2 = load_or_generate_keypair(dir.path()).expect("load");
        assert_eq!(kp1, kp2, "keypair should be stable across calls");
        assert_eq!(kp1.len(), 64);
    }
}
