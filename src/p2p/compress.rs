//! Compression using Zstandard (zstd) at level 19.
//!
//! Level 19 gives the best ratio:decompression-speed tradeoff for P2P:
//! - Compression is one-shot (CPU cost acceptable)
//! - Decompression at the receiver is fast (~1 GB/s)
//!
//! The async wrappers run inside `spawn_blocking` so they never freeze egui.

use crate::p2p::P2pError;

// ── Sync variants (used in tests and from blocking contexts) ─────────────────

pub fn compress_sync(data: &[u8]) -> crate::p2p::Result<Vec<u8>> {
    zstd::encode_all(data, 19).map_err(|e| P2pError::Compress(e.to_string()))
}

pub fn decompress_sync(data: &[u8]) -> crate::p2p::Result<Vec<u8>> {
    zstd::decode_all(data).map_err(|e| P2pError::Compress(e.to_string()))
}

// ── Async variants (safe to call from tokio tasks) ───────────────────────────

pub async fn compress(data: Vec<u8>) -> crate::p2p::Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || compress_sync(&data))
        .await
        .map_err(|e| P2pError::Compress(e.to_string()))?
}

pub async fn decompress(data: Vec<u8>) -> crate::p2p::Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || decompress_sync(&data))
        .await
        .map_err(|e| P2pError::Compress(e.to_string()))?
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_sync() {
        let original = b"hello world this is a test payload for zstd compression 1234567890";
        let compressed = compress_sync(original).expect("compress_sync failed");
        let decompressed = decompress_sync(&compressed).expect("decompress_sync failed");
        assert_eq!(original.as_slice(), decompressed.as_slice());
    }

    #[tokio::test]
    async fn round_trip_async() {
        let original = b"async round trip test".to_vec();
        let compressed = compress(original.clone()).await.expect("compress failed");
        let decompressed = decompress(compressed).await.expect("decompress failed");
        assert_eq!(original, decompressed);
    }
}
