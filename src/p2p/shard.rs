//! Sharding: split compressed data into fixed-size chunks,
//! hash each one with SHA-256, and reconstruct + verify.

use crate::p2p::{P2pError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardMeta {
    pub index: u32,
    pub hash_hex: String,
    pub size: usize,
}

/// Manifest describing a sharded, compressed dataset.
/// `manifest_hash` is the SHA-256 of the JSON-serialized manifest
/// **without** the `manifest_hash` field itself (computed last).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardManifest {
    pub id: String,
    pub name: String,
    pub original_size: usize,
    pub compressed_size: usize,
    pub total_shards: u32,
    pub shards: Vec<ShardMeta>,
    /// SHA-256 of the manifest before this field was added (integrity seal).
    pub manifest_hash: String,
}

// ── Splitting ────────────────────────────────────────────────────────────────

/// Default shard size: 512 KiB. Small enough to retry individually over P2P.
pub const DEFAULT_SHARD_SIZE: usize = 512 * 1024;

/// Split `compressed_data` into shards of `shard_size` bytes.
/// Returns the manifest (without saving to disk) and the shard payloads.
pub fn split(
    id: &str,
    name: &str,
    original_size: usize,
    compressed_data: &[u8],
    shard_size: usize,
) -> (ShardManifest, Vec<Vec<u8>>) {
    let mut shards_data: Vec<Vec<u8>> = Vec::new();
    let mut shard_metas: Vec<ShardMeta> = Vec::new();

    for (index, chunk) in compressed_data.chunks(shard_size).enumerate() {
        let hash_hex = sha256_hex(chunk);
        shard_metas.push(ShardMeta {
            index: index as u32,
            hash_hex,
            size: chunk.len(),
        });
        shards_data.push(chunk.to_vec());
    }

    // Build manifest without manifest_hash first, then compute it.
    let partial = ShardManifest {
        id: id.to_string(),
        name: name.to_string(),
        original_size,
        compressed_size: compressed_data.len(),
        total_shards: shard_metas.len() as u32,
        shards: shard_metas,
        manifest_hash: String::new(), // placeholder
    };

    let manifest_hash = hash_manifest_without_self(&partial);
    let manifest = ShardManifest {
        manifest_hash,
        ..partial
    };

    (manifest, shards_data)
}

// ── Reconstruction ───────────────────────────────────────────────────────────

/// Reassemble shards into the original compressed payload.
/// Verifies:
///   1. Manifest hash (against MITM replacement of the manifest)
///   2. Per-shard SHA-256 (against corruption / partial delivery)
pub fn reconstruct(manifest: &ShardManifest, shards: &[Vec<u8>]) -> Result<Vec<u8>> {
    // 1. Verify manifest integrity
    let expected_mhash = hash_manifest_without_self(manifest);
    if expected_mhash != manifest.manifest_hash {
        return Err(P2pError::Integrity(
            "manifest hash mismatch — possible tampering".into(),
        ));
    }

    // 2. Verify shard count
    if shards.len() != manifest.total_shards as usize {
        return Err(P2pError::Integrity(format!(
            "expected {} shards, got {}",
            manifest.total_shards,
            shards.len()
        )));
    }

    // 3. Verify each shard and reassemble
    let mut output = Vec::with_capacity(manifest.compressed_size);
    for meta in &manifest.shards {
        let shard = shards
            .get(meta.index as usize)
            .ok_or_else(|| P2pError::Integrity(format!("missing shard {}", meta.index)))?;

        let actual_hash = sha256_hex(shard);
        if actual_hash != meta.hash_hex {
            return Err(P2pError::Integrity(format!(
                "shard {} hash mismatch: expected {}, got {}",
                meta.index, meta.hash_hex, actual_hash
            )));
        }

        output.extend_from_slice(shard);
    }

    Ok(output)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Hash the manifest as JSON, but with `manifest_hash` set to empty string,
/// so the calculation is stable regardless of the field's actual value.
fn hash_manifest_without_self(m: &ShardManifest) -> String {
    let stable = ShardManifest {
        manifest_hash: String::new(),
        ..m.clone()
    };
    let json = serde_json::to_string(&stable).unwrap_or_default();
    sha256_hex(json.as_bytes())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::compress::{compress_sync, decompress_sync};

    const SAMPLE_JSON: &[u8] = br#"[{"id":1,"name":"Alice"},{"id":2,"name":"Bob"}]"#;

    #[test]
    fn p2p_pipeline_full() {
        // Step 1: compress
        let compressed = compress_sync(SAMPLE_JSON).expect("compress failed");
        assert!(
            compressed.len() < SAMPLE_JSON.len() || SAMPLE_JSON.len() < 100,
            "compression should reduce size for non-trivial payloads"
        );

        // Step 2: split into shards (use tiny shard size to test multi-shard)
        let id = "test-dataset-001";
        let (manifest, shards) = split(id, "test", SAMPLE_JSON.len(), &compressed, 16);

        assert_eq!(manifest.total_shards, shards.len() as u32);
        assert!(manifest.total_shards >= 1);
        // Verify each shard hash
        for (meta, shard) in manifest.shards.iter().zip(shards.iter()) {
            assert_eq!(meta.hash_hex, sha256_hex(shard));
        }

        // Step 3: reconstruct
        let reconstructed = reconstruct(&manifest, &shards).expect("reconstruct failed");
        assert_eq!(reconstructed, compressed);

        // Step 4: decompress
        let decompressed = decompress_sync(&reconstructed).expect("decompress failed");
        assert_eq!(decompressed, SAMPLE_JSON);
    }

    #[test]
    fn p2p_pipeline_tampered_shard_detected() {
        let compressed = compress_sync(SAMPLE_JSON).expect("compress");
        let (manifest, mut shards) = split("x", "test", SAMPLE_JSON.len(), &compressed, 8);
        // Corrupt the first shard
        if let Some(s) = shards.first_mut() {
            *s = b"corrupted!".to_vec();
        }
        let result = reconstruct(&manifest, &shards);
        assert!(result.is_err(), "should detect corrupted shard");
    }

    #[test]
    fn p2p_pipeline_tampered_manifest_detected() {
        let compressed = compress_sync(SAMPLE_JSON).expect("compress");
        let (mut manifest, shards) = split("x", "test", SAMPLE_JSON.len(), &compressed, 8);
        manifest.name = "evil_tampered".into();
        let result = reconstruct(&manifest, &shards);
        assert!(result.is_err(), "should detect tampered manifest");
    }
}
