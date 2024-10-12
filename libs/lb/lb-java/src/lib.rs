use jni::{objects::JClass, sys::jstring, JNIEnv};

#[no_mangle]
pub extern "system" fn Java_org_example_Library_hello<'local>(
    env: JNIEnv<'local>, _class: JClass<'local>,
) -> jstring {
    let output = env
        .new_string("This is a test from beautiful Rust!")
        .unwrap();
    output.into_raw()
}
