mod java_utils;

use java_utils::{populate_err, rstring};
use jni::{
    objects::{JClass, JString, JValue},
    sys::{jlong, jobject},
    JNIEnv,
};
use lb_rs::{blocking::Lb, model::core_config::Config};

#[no_mangle]
pub extern "system" fn Java_net_lockbook_Lb_init<'local>(
    mut env: JNIEnv<'local>, _class: JClass<'local>, input: JString<'local>,
) -> jobject {
    let writeable_path = rstring(&mut env, &input);
    let config = Config { logs: true, colored_logs: false, writeable_path };

    let res = env
        .find_class("net/lockbook/InitRes")
        .expect("Class Not Found");
    let obj = env.alloc_object(res).unwrap();

    match Lb::init(config) {
        Ok(lb) => {
            let ptr = Box::into_raw(Box::new(lb)) as jlong;
            env.set_field(&obj, "lb", "J", JValue::Long(ptr)).unwrap();
        }
        Err(err) => {
            populate_err(&mut env, &obj, err);
        }
    };

    obj.into_raw()
}
