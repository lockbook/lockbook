use lockbook_core::service::file_compression_service;

#[test]
fn compress_decompress() {
    assert_eq!(
        file_compression_service::decompress(
            &file_compression_service::compress(b"hello").unwrap(),
        )
        .unwrap(),
        b"hello"
    );
}
