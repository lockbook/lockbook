#[test]
fn ffi_api_location_matches() {
    assert_eq!(
        lb::DEFAULT_API_LOCATION,
        std::ffi::CStr::from_bytes_with_nul(crate::LB_DEFAULT_API_LOCATION)
            .unwrap()
            .to_str()
            .unwrap()
    )
}

#[test]
fn ffi_enums_match() {
    use lb::SupportedImageFormats as ImgFmts;
    assert_eq!(ImgFmts::Png as u8, 0);
    assert_eq!(ImgFmts::Jpeg as u8, 1);
    assert_eq!(ImgFmts::Pnm as u8, 2);
    assert_eq!(ImgFmts::Tga as u8, 3);
    assert_eq!(ImgFmts::Farbfeld as u8, 4);
    assert_eq!(ImgFmts::Bmp as u8, 5);

    use lb::GooglePlayAccountState as Gp;
    assert_eq!(Gp::Ok as u8, 0);
    assert_eq!(Gp::Canceled as u8, 1);
    assert_eq!(Gp::GracePeriod as u8, 2);
    assert_eq!(Gp::OnHold as u8, 3);

    use lb::AppStoreAccountState as Ap;
    assert_eq!(Ap::Ok as u8, 0);
    assert_eq!(Ap::GracePeriod as u8, 1);
    assert_eq!(Ap::FailedToRenew as u8, 2);
    assert_eq!(Ap::Expired as u8, 3);
}
