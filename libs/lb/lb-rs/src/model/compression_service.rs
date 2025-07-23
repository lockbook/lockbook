use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

use super::errors::{LbErrKind, LbResult};

pub fn compress(content: &[u8]) -> LbResult<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
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

#[test]
fn compress_decompress() {
    assert_eq!(decompress(&compress(b"hello").unwrap()).unwrap(), b"hello");
}
