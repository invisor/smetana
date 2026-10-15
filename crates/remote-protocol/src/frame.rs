//! Frames: a `u32` big-endian length, then that many bytes of postcard.
//!
//! The length is checked against `MAX_FRAME` before a single byte of the body
//! is read or allocated, so a peer cannot make the other end reserve memory by
//! claiming a large frame.

use iroh::endpoint::{RecvStream, SendStream};
use serde::{de::DeserializeOwned, Serialize};

/// The largest body either end will accept, in bytes. A reply to `tracker_snapshot`
/// is a whole board and can pass a megabyte, so this is 16 MiB; the readers that
/// face a stranger (the first frame of a connection, a phone's requests) pass a
/// much smaller `limit` of their own, and a lying length costs nothing either way
/// because it is checked before the body is allocated.
pub const MAX_FRAME: usize = 16 << 20;

#[derive(Debug, PartialEq, Eq)]
pub enum FrameError {
    /// The length prefix is larger than the limit given.
    TooLarge(usize),
    /// The bytes ended before the frame did.
    Truncated,
    /// The body is not the type asked for.
    Decode(String),
    /// The body could not be written as postcard.
    Encode(String),
    /// The stream failed or ended.
    Io(String),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::TooLarge(n) => write!(f, "frame of {n} bytes is over the limit"),
            FrameError::Truncated => write!(f, "frame is cut short"),
            FrameError::Decode(e) => write!(f, "frame does not decode: {e}"),
            FrameError::Encode(e) => write!(f, "frame does not encode: {e}"),
            FrameError::Io(e) => write!(f, "stream error: {e}"),
        }
    }
}

impl std::error::Error for FrameError {}

/// A value as one frame.
pub fn encode_frame<T: Serialize>(value: &T) -> Result<Vec<u8>, FrameError> {
    let body = postcard::to_stdvec(value).map_err(|e| FrameError::Encode(e.to_string()))?;
    if body.len() > MAX_FRAME {
        return Err(FrameError::TooLarge(body.len()));
    }
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// The first frame of `bytes` as a value, and whatever follows it.
pub fn decode_frame<T: DeserializeOwned>(bytes: &[u8]) -> Result<(T, &[u8]), FrameError> {
    let Some((prefix, rest)) = bytes.split_first_chunk::<4>() else {
        return Err(FrameError::Truncated);
    };
    let len = u32::from_be_bytes(*prefix) as usize;
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge(len));
    }
    if rest.len() < len {
        return Err(FrameError::Truncated);
    }
    let (body, rest) = rest.split_at(len);
    let value = postcard::from_bytes(body).map_err(|e| FrameError::Decode(e.to_string()))?;
    Ok((value, rest))
}

/// Writes one frame to a stream. It does not finish the stream: the stream
/// outlives the exchange once the bridge exists.
pub async fn write_frame<T: Serialize>(send: &mut SendStream, value: &T) -> Result<(), FrameError> {
    let frame = encode_frame(value)?;
    send.write_all(&frame).await.map_err(|e| FrameError::Io(e.to_string()))
}

/// Reads one frame from a stream, refusing anything over `limit` bytes before
/// reading the body.
pub async fn read_frame<T: DeserializeOwned>(
    recv: &mut RecvStream,
    limit: usize,
) -> Result<T, FrameError> {
    let mut prefix = [0u8; 4];
    recv.read_exact(&mut prefix).await.map_err(|e| FrameError::Io(e.to_string()))?;
    let len = u32::from_be_bytes(prefix) as usize;
    if len > limit.min(MAX_FRAME) {
        return Err(FrameError::TooLarge(len));
    }
    let mut body = vec![0u8; len];
    recv.read_exact(&mut body).await.map_err(|e| FrameError::Io(e.to_string()))?;
    postcard::from_bytes(&body).map_err(|e| FrameError::Decode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_a_big_endian_length_and_a_body() {
        let frame = encode_frame(&"hi".to_string()).unwrap();
        assert_eq!(&frame[..4], &[0, 0, 0, 3]);
        assert_eq!(frame.len(), 4 + 3);
    }

    #[test]
    fn a_short_or_oversized_frame_is_an_error_and_not_a_panic() {
        assert_eq!(decode_frame::<String>(&[0, 0]), Err(FrameError::Truncated));
        assert_eq!(decode_frame::<String>(&[0, 0, 0, 9, 1]), Err(FrameError::Truncated));
        assert!(matches!(
            decode_frame::<String>(&[0xff, 0xff, 0xff, 0xff]),
            Err(FrameError::TooLarge(_))
        ));
        // A one-byte body of 0xff is a varint that never ends.
        assert!(matches!(decode_frame::<u64>(&[0, 0, 0, 1, 0xff]), Err(FrameError::Decode(_))));
    }

    #[test]
    fn bytes_after_a_frame_are_handed_back() {
        let mut bytes = encode_frame(&1u8).unwrap();
        bytes.extend_from_slice(&[9, 9]);
        let (value, rest) = decode_frame::<u8>(&bytes).unwrap();
        assert_eq!(value, 1);
        assert_eq!(rest, &[9, 9]);
    }
}
