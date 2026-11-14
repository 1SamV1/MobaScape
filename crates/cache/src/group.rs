use std::io::{Cursor, Read};

use crate::error::{Error, Result};
use crate::xtea;

#[derive(Debug, Clone)]
pub struct Group {
    pub compression: u8,
    pub decompressed_len: u32,
    pub data: Vec<u8>,
}

/// Decode one raw group. `raw` is the sector-chain payload as stored in the
/// index entry; `key` is present for XTEA-encrypted map groups.
pub fn decode(raw: &[u8], key: Option<[u32; 4]>) -> Result<Group> {
    if raw.len() < 9 {
        return Err(Error::parse(format!(
            "group too short: {} bytes (need at least 9)",
            raw.len()
        )));
    }

    let compression = raw[0];
    let compressed_len = u32::from_be_bytes([raw[1], raw[2], raw[3], raw[4]]) as usize;

    // Everything from the compressed-length field onward is encrypted for map
    // groups, including the 4-byte decompressed length.
    let mut body = raw[5..].to_vec();
    if let Some(key) = key {
        xtea::decrypt(&mut body, key);
    }

    if compression == 0 {
        // Uncompressed groups have no decompressed-length header. They store
        // `compressed_len` bytes of data directly after the length field and a
        // two-byte version trailer at the end of the group.
        let payload = body.get(..compressed_len).ok_or_else(|| {
            Error::parse(format!(
                "uncompressed payload out of range: need {compressed_len}, have {}",
                body.len()
            ))
        })?;
        return Ok(Group {
            compression,
            decompressed_len: compressed_len as u32,
            data: payload.to_vec(),
        });
    }

    if body.len() < 4 {
        return Err(Error::parse("group body missing decompressed length"));
    }
    let decompressed_len = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);

    let end = 4usize
        .checked_add(compressed_len)
        .ok_or_else(|| Error::parse("compressed length overflow"))?;
    let payload = body.get(4..end).ok_or_else(|| {
        Error::parse(format!(
            "compressed payload out of range: need {end}, have {}",
            body.len()
        ))
    })?;

    let data = match compression {
        1 => decompress_bzip2(payload)?,
        2 => decompress_gzip(payload)?,
        3 => decompress_lzma(payload)?,
        other => return Err(Error::UnsupportedCompression(other)),
    };

    if data.len() != decompressed_len as usize {
        return Err(Error::parse(format!(
            "decompressed length mismatch: header says {decompressed_len}, got {} (compression {compression})",
            data.len()
        )));
    }

    Ok(Group {
        compression,
        decompressed_len,
        data,
    })
}

/// OSRS strips the four-byte `BZh<level>` stream header from bzip2 payloads.
fn decompress_bzip2(payload: &[u8]) -> Result<Vec<u8>> {
    let mut stream = Vec::with_capacity(payload.len() + 4);
    stream.extend_from_slice(b"BZh9");
    stream.extend_from_slice(payload);

    let mut out = Vec::new();
    bzip2::read::BzDecoder::new(Cursor::new(stream))
        .read_to_end(&mut out)
        .map_err(|e| Error::parse(format!("bzip2: {e}")))?;
    Ok(out)
}

/// The cache stores gzip streams with a damaged/foreign 8-byte trailer, so
/// decompress the deflate payload directly and ignore the trailer.
fn decompress_gzip(payload: &[u8]) -> Result<Vec<u8>> {
    if payload.len() < 18 || payload[0] != 0x1f || payload[1] != 0x8b {
        return Err(Error::parse("gzip: bad magic"));
    }
    if payload[2] != 8 {
        return Err(Error::parse(format!(
            "gzip: unsupported method {}",
            payload[2]
        )));
    }

    let flags = payload[3];
    let mut pos = 10usize;

    if flags & 0x04 != 0 {
        if payload.len() < pos + 2 {
            return Err(Error::parse("gzip: truncated extra length"));
        }
        let xlen = u16::from_le_bytes([payload[pos], payload[pos + 1]]) as usize;
        pos += 2 + xlen;
    }
    if flags & 0x08 != 0 {
        pos = find_zero(payload, pos)? + 1;
    }
    if flags & 0x10 != 0 {
        pos = find_zero(payload, pos)? + 1;
    }
    if flags & 0x02 != 0 {
        pos += 2;
    }

    if pos >= payload.len() {
        return Err(Error::parse("gzip: no deflate payload"));
    }

    let mut out = Vec::new();
    flate2::read::DeflateDecoder::new(&payload[pos..])
        .read_to_end(&mut out)
        .map_err(|e| Error::parse(format!("gzip deflate: {e}")))?;
    Ok(out)
}

fn find_zero(data: &[u8], start: usize) -> Result<usize> {
    data.get(start..)
        .and_then(|s| s.iter().position(|&b| b == 0))
        .map(|i| start + i)
        .ok_or_else(|| Error::parse("gzip: unterminated string field"))
}

fn decompress_lzma(payload: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    lzma_rs::lzma_decompress(&mut Cursor::new(payload), &mut out)
        .map_err(|e| Error::parse(format!("lzma: {e}")))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_group() {
        assert!(decode(&[0u8; 4], None).is_err());
    }

    #[test]
    fn decodes_uncompressed_group_with_version_trailer() {
        // Uncompressed groups store the payload directly after the length
        // field and finish with a two-byte version trailer. They do not have
        // the four-byte decompressed-length header used by compressed groups.
        let raw = [0, 0, 0, 0, 3, 10, 20, 30, 0, 2];
        let group = decode(&raw, None).unwrap();
        assert_eq!(group.data, vec![10, 20, 30]);
        assert_eq!(group.decompressed_len, 3);
    }
}
