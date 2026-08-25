//! The loopback wire protocol this process speaks to its parent.
//!
//! Independently implemented from (not sharing a crate with) the client
//! side in `envryn_core::ai::worker_client` -- this crate must not depend on
//! `envryn-core` at all (AI-INV-001/002/004/005: no vault type may be
//! reachable from here even transitively). Framing mirrors
//! `envryn_core::sync::protocol`'s length-prefixed JSON, since it is the
//! same kind of problem, but the two implementations are deliberately
//! separate code.

use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

/// Generous for a single prompt/response; a hostile or buggy peer sending
/// more is refused, not allocated for.
const MAX_MESSAGE_LEN: u32 = 4 * 1024 * 1024;

#[derive(Deserialize)]
pub struct Request {
    pub token: String,
    pub prompt: String,
    pub max_tokens: u32,
    /// Which known schema (if any) the response must conform to -- a plain
    /// string tag, not a shared enum type, since this crate must not depend
    /// on `envryn-core` at all (AI-INV-001/002/004/005): the wire format
    /// itself is the only contract with `envryn_core::ai::worker_client`,
    /// which sends this field's wire spelling from its own
    /// `SchemaKind`. Absent (or any value this binary does not recognise)
    /// means ordinary unconstrained generation -- a client built against a
    /// newer schema name than this worker understands degrades to the
    /// existing prompting-only behaviour rather than failing the request.
    #[serde(default)]
    pub schema: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Ok { text: String },
    Error { message: String },
}

pub fn write_json<W: Write, T: Serialize>(stream: &mut W, value: &T) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let len = u32::try_from(bytes.len())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "message too large"))?;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(&bytes)
}

pub fn read_json<R: Read, T: for<'de> Deserialize<'de>>(stream: &mut R) -> std::io::Result<T> {
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes);
    if len > MAX_MESSAGE_LEN {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "peer sent an oversized message",
        ));
    }
    let mut buf = vec![0u8; len as usize];
    stream.read_exact(&mut buf)?;
    serde_json::from_slice(&buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
