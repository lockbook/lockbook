extern crate reqwest;

use std::ffi::{CStr, CString};
use std::fs::File;
use std::os::raw::c_char;
use std::io::Write;

#[no_mangle]
pub unsafe extern "C" fn hello(to: *const c_char) -> *mut c_char {
    let c_str = CStr::from_ptr(to);
    let document_path = c_str.to_str().expect("cstring");
    let path = format!("{}{}", document_path, "test").replace("file://", "");
    println!("path: {}", path);

    let mut file = File::create(path).expect("failed to write file");
    file.write_all(b"Hello, World!").expect("failed to write hello");

    CString::new(request())
        .unwrap()
        .into_raw()
}

fn request() -> String {
    let a = reqwest::get("https://httpbin.org/get")
        .expect("fail1")
        .text()
        .expect("fail2");

    println!("{}", a);

    a
}

#[no_mangle]
pub unsafe extern "C" fn hello_release(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    CString::from_raw(s);
}
