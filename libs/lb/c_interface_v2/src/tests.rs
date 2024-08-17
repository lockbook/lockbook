#[test]
fn ffi_api_location_matches() {
    assert_eq!(
        lb_rs::DEFAULT_API_LOCATION,
        std::ffi::CStr::from_bytes_with_nul(crate::LB_DEFAULT_API_LOCATION)
            .unwrap()
            .to_str()
            .unwrap()
    )
}

#[test]
fn ffi_enums_match() {
    use lb_rs::GooglePlayAccountState as Gp;
    assert_eq!(Gp::Ok as u8, 0);
    assert_eq!(Gp::Canceled as u8, 1);
    assert_eq!(Gp::GracePeriod as u8, 2);
    assert_eq!(Gp::OnHold as u8, 3);

    use lb_rs::AppStoreAccountState as Ap;
    assert_eq!(Ap::Ok as u8, 0);
    assert_eq!(Ap::GracePeriod as u8, 1);
    assert_eq!(Ap::FailedToRenew as u8, 2);
    assert_eq!(Ap::Expired as u8, 3);
}
