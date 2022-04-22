use std::io::{Read, Write};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::CoreError;

pub fn compress(content: &[u8]) -> Result<Vec<u8>, CoreError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(content).map_err(CoreError::from)?;
    encoder.finish().map_err(CoreError::from)
}

pub fn decompress(content: &[u8]) -> Result<Vec<u8>, CoreError> {
    let mut decoder = ZlibDecoder::new(content);
    let mut result = Vec::<u8>::new();
    decoder.read_to_end(&mut result).map_err(CoreError::from)?;
    Ok(result)
}
