use lockbook_shared::compression_service;

#[test]
fn compress_decompress() {
    assert_eq!(
        compression_service::decompress(&compression_service::compress(b"hello").unwrap(),)
            .unwrap(),
        b"hello"
    );
}
