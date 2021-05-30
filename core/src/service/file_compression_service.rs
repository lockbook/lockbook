use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};

pub fn compress(content: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(content)?;
    encoder.finish()
}

pub fn decompress(content: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = ZlibDecoder::new(content);
    let mut result = Vec::<u8>::new();
    decoder.read_to_end(&mut result)?;
    Ok(result)
}

#[cfg(test)]
mod unit_tests {
    use crate::service::file_compression_service;

    #[test]
    fn compress_decompress() {
        file_compression_service::decompress(
            &file_compression_service::compress(b"hello").unwrap(),
        )
        .unwrap();
    }
}
