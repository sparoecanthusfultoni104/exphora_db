//! ShareLink: a compact magnet-link-style descriptor for a P2P dataset.
//!
//! A link is a base64url-encoded JSON blob containing everything the receiver
//! needs to locate, authenticate and verify the dataset:
//!
//! ```text
//! exphora:eyJtYW5pZmVzdF9oYXNoIjoiLi4uIn0...
//!          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//!          base64url JSON (no padding)
//! ```
//!
//! **Why not DHT or mDNS for v1?**
//! The target audience (pentesters, security researchers) actively avoids DHTs
//! because participation leaks IPs to any node querying the DHT. Manual link
//! exchange gives full control over who knows what.

use crate::p2p::shard::ShardManifest;
use crate::p2p::{P2pError, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};

pub const LINK_PREFIX: &str = "exphora:";

// ── ShareLink ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShareLink {
    /// SHA-256 of the ShardManifest (verifies manifest integrity on receipt).
    pub manifest_hash: String,
    /// SHA-256 of each shard, in order (allows per-shard pre-validation).
    pub shard_hashes: Vec<String>,
    /// Secret token validated by the server before serving any shard.
    pub auth_token: String,
    /// One or more "host:port" addresses where the seeder is reachable.
    pub seeder_addrs: Vec<String>,
    /// base64url of the seeder's Noise static public key (for Noise XX handshake).
    pub seeder_noise_pubkey: String,
}

// ── Generation ────────────────────────────────────────────────────────────────

pub fn generate_link(
    manifest: &ShardManifest,
    auth_token: &str,
    seeder_addrs: &[impl AsRef<str>],
    noise_pubkey_b64: &str,
) -> String {
    let link = ShareLink {
        manifest_hash: manifest.manifest_hash.clone(),
        shard_hashes: manifest.shards.iter().map(|s| s.hash_hex.clone()).collect(),
        auth_token: auth_token.to_string(),
        seeder_addrs: seeder_addrs
            .iter()
            .map(|a| a.as_ref().to_string())
            .collect(),
        seeder_noise_pubkey: noise_pubkey_b64.to_string(),
    };

    let json = serde_json::to_string(&link).unwrap_or_default();
    format!("{}{}", LINK_PREFIX, URL_SAFE_NO_PAD.encode(json.as_bytes()))
}

// ── Parsing ───────────────────────────────────────────────────────────────────

pub fn parse_link(s: &str) -> Result<ShareLink> {
    let encoded = s
        .strip_prefix(LINK_PREFIX)
        .ok_or_else(|| P2pError::Discovery(format!("missing '{LINK_PREFIX}' prefix")))?;

    let json_bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|e| P2pError::Discovery(format!("base64 decode: {e}")))?;

    let link: ShareLink = serde_json::from_slice(&json_bytes)
        .map_err(|e| P2pError::Discovery(format!("JSON parse: {e}")))?;

    // Basic validation
    if link.manifest_hash.is_empty() {
        return Err(P2pError::Discovery("manifest_hash is empty".into()));
    }
    if link.auth_token.is_empty() {
        return Err(P2pError::Discovery("auth_token is empty".into()));
    }
    if link.seeder_noise_pubkey.is_empty() {
        return Err(P2pError::Discovery("seeder_noise_pubkey is empty".into()));
    }

    Ok(link)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::compress::compress_sync;
    use crate::p2p::shard::split;

    fn make_test_manifest() -> ShardManifest {
        let data = compress_sync(b"test payload for discovery").unwrap();
        let (m, _) = split("disc-id", "test", 25, &data, 8);
        m
    }

    #[test]
    fn generate_and_parse_roundtrip() {
        let manifest = make_test_manifest();
        let link = generate_link(
            &manifest,
            "secret-token-abc",
            &["192.168.1.100:7878", "10.0.0.5:7878"],
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        );

        assert!(link.starts_with(LINK_PREFIX));

        let parsed = parse_link(&link).expect("parse failed");
        assert_eq!(parsed.manifest_hash, manifest.manifest_hash);
        assert_eq!(parsed.auth_token, "secret-token-abc");
        assert_eq!(parsed.seeder_addrs.len(), 2);
        assert_eq!(parsed.shard_hashes.len(), manifest.total_shards as usize);
    }

    #[test]
    fn parse_invalid_prefix() {
        let result = parse_link("magnet:?xt=urn:...");
        assert!(result.is_err());
    }

    #[test]
    fn parse_invalid_base64() {
        let result = parse_link("exphora:!!!not_base64!!!");
        assert!(result.is_err());
    }
}
