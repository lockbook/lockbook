use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

use super::errors::{LbErrKind, LbResult};

/// How many bytes of the payload to sniff for binary detection. Matches
/// git's `buffer_is_binary`: a NUL byte in the first ~8 KiB is a strong
/// signal the bytes won't deflate, so we skip compression.
const SNIFF_BYTES: usize = 8000;

const MIN_COMPRESS_SIZE: usize = 256;

/// Pick a compression level by looking at the bytes we're about to write.
/// `None` for binary-looking or tiny payloads; `Default` otherwise.
pub fn level_for_content(content: &[u8]) -> Compression {
    if content.len() < MIN_COMPRESS_SIZE {
        debug!("no compression");
        return Compression::none();
    }
    let sniff = &content[..content.len().min(SNIFF_BYTES)];
    if sniff.contains(&0u8) {
        debug!("no compression");
        return Compression::none();
    }

    debug!("compression used");
    Compression::default()
}

pub fn compress(content: &[u8]) -> LbResult<Vec<u8>> {
    let level = level_for_content(content);
    let mut encoder = ZlibEncoder::new(Vec::new(), level);
    encoder
        .write_all(content)
        .map_err(|err| LbErrKind::Unexpected(format!("unexpected compression error: {err:?}")))?;

    Ok(encoder
        .finish()
        .map_err(|err| LbErrKind::Unexpected(format!("unexpected compression error: {err:?}")))?)
}

pub fn decompress(content: &[u8]) -> LbResult<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(content);
    let mut result = Vec::<u8>::new();
    decoder
        .read_to_end(&mut result)
        .map_err(|err| LbErrKind::Unexpected(format!("unexpected compression error: {err:?}")))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_text() {
        let original = "hello world ".repeat(64);
        let original = original.as_bytes();
        let round_tripped = decompress(&compress(original).unwrap()).unwrap();
        assert_eq!(round_tripped, original);
    }

    #[test]
    fn roundtrips_binary_with_nul_bytes() {
        let original: Vec<u8> = (0u32..4096).map(|i| (i.wrapping_mul(31) ^ 0x55) as u8).collect();
        assert!(original.contains(&0u8), "expected sniff to see a NUL");
        let round_tripped = decompress(&compress(&original).unwrap()).unwrap();
        assert_eq!(round_tripped, original);
    }

    #[test]
    fn roundtrips_small_payload() {
        let original = b"tiny";
        let round_tripped = decompress(&compress(original).unwrap()).unwrap();
        assert_eq!(round_tripped, original);
    }

    #[test]
    fn level_none_for_small_payload() {
        let small = vec![b'a'; MIN_COMPRESS_SIZE - 1];
        assert_eq!(level_for_content(&small).level(), Compression::none().level());
    }

    #[test]
    fn level_none_when_nul_inside_sniff_window() {
        let mut content = vec![b'a'; SNIFF_BYTES * 2];
        content[100] = 0;
        assert_eq!(level_for_content(&content).level(), Compression::none().level());
    }

    #[test]
    fn level_default_for_text() {
        let content = "the quick brown fox ".repeat(64);
        assert_eq!(level_for_content(content.as_bytes()).level(), Compression::default().level());
    }

    #[test]
    fn nul_after_sniff_window_does_not_skip_compression() {
        // Only what's in the first SNIFF_BYTES matters.
        let mut content = vec![b'a'; SNIFF_BYTES + 1024];
        content[SNIFF_BYTES + 500] = 0;
        assert_eq!(level_for_content(&content).level(), Compression::default().level());
    }
}
