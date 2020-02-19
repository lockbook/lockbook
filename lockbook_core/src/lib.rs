extern crate reqwest;

use std::ffi::{CStr};
use std::os::raw::{c_char, c_int};

#[no_mangle]
pub unsafe extern "C" fn create_account(c_username: *const c_char) -> c_int {
    let username = CStr::from_ptr(c_username).to_str()
        .expect("Could not C String -> Rust String");

    println!("username: {}", username);

    1
}
