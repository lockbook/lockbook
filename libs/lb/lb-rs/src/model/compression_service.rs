use std::io::{Read, Write};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::model::{SharedErrorKind, SharedResult};

pub fn compress(content: &[u8]) -> SharedResult<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(content)
        .map_err(|_| SharedErrorKind::Unexpected("unexpected compression error"))?;
    encoder
        .finish()
        .map_err(|_| SharedErrorKind::Unexpected("unexpected compression error").into())
}

pub fn decompress(content: &[u8]) -> SharedResult<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(content);
    let mut result = Vec::<u8>::new();
    decoder
        .read_to_end(&mut result)
        .map_err(|_| SharedErrorKind::Unexpected("unexpected decompression error"))?;
    Ok(result)
}

#[test]
fn compress_decompress() {
    assert_eq!(decompress(&compress(b"hello").unwrap()).unwrap(), b"hello");
}
