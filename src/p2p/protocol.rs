//! Wire protocol for P2P shard exchange.
//!
//! Frame format:
//! ```text
//! ┌──────────┬──────────────┬────────────┐
//! │ type (1B)│  length (4B) │  payload   │
//! │  u8 BE   │   u32 BE     │  (JSON)    │
//! └──────────┴──────────────┴────────────┘
//! ```
//!
//! The 1-byte type discriminator allows future message types without breaking
//! existing peers (unknown types can be skipped by reading `length` bytes).

use crate::p2p::{P2pError, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ── Message types ─────────────────────────────────────────────────────────────

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    ShardRequest = 0x01,
    ShardResponse = 0x02,
    Error = 0xFF,
}

impl TryFrom<u8> for MsgType {
    type Error = P2pError;
    fn try_from(v: u8) -> Result<Self> {
        match v {
            0x01 => Ok(MsgType::ShardRequest),
            0x02 => Ok(MsgType::ShardResponse),
            0xFF => Ok(MsgType::Error),
            other => Err(P2pError::Transfer(format!(
                "unknown msg type: 0x{other:02X}"
            ))),
        }
    }
}

// ── Payloads ──────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct ShardRequest {
    pub shard_hash: String,
    pub auth_token: String,
}

#[derive(Serialize, Deserialize)]
pub struct ShardResponse {
    /// Raw shard bytes encoded as hex (keeps JSON validity over binary).
    pub data_hex: String,
}

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    pub message: String,
}

// ── Framing ────────────────────────────────────────────────────────────────────

/// Send a framed message: [1B type][4B len BE][payload].
pub async fn send_msg<W, P>(writer: &mut W, msg_type: MsgType, payload: &P) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
    P: Serialize,
{
    let json = serde_json::to_vec(payload).map_err(|e| P2pError::Transfer(e.to_string()))?;

    let len = json.len() as u32;
    writer
        .write_u8(msg_type as u8)
        .await
        .map_err(P2pError::Io)?;
    writer.write_u32(len).await.map_err(P2pError::Io)?;
    writer.write_all(&json).await.map_err(P2pError::Io)?;
    Ok(())
}

/// Receive a framed message and return its type + raw JSON payload.
pub async fn recv_msg<R>(reader: &mut R) -> Result<(MsgType, Vec<u8>)>
where
    R: AsyncReadExt + Unpin,
{
    let type_byte = reader.read_u8().await.map_err(P2pError::Io)?;
    let msg_type = MsgType::try_from(type_byte)?;

    let len = reader.read_u32().await.map_err(P2pError::Io)? as usize;
    const MAX_PAYLOAD: usize = 64 * 1024 * 1024; // 64 MiB guard
    if len > MAX_PAYLOAD {
        return Err(P2pError::Transfer(format!(
            "payload too large: {len} bytes"
        )));
    }

    let mut payload = vec![0u8; len];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(P2pError::Io)?;
    Ok((msg_type, payload))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn frame_roundtrip() {
        let req = ShardRequest {
            shard_hash: "abc123".into(),
            auth_token: "secret".into(),
        };

        let mut buf: Vec<u8> = Vec::new();
        send_msg(&mut buf, MsgType::ShardRequest, &req)
            .await
            .expect("send_msg failed");

        let mut reader = BufReader::new(buf.as_slice());
        let (msg_type, payload) = recv_msg(&mut reader).await.expect("recv_msg failed");

        assert_eq!(msg_type, MsgType::ShardRequest);
        let decoded: ShardRequest = serde_json::from_slice(&payload).unwrap();
        assert_eq!(decoded.shard_hash, "abc123");
        assert_eq!(decoded.auth_token, "secret");
    }
}
