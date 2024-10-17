use jni::{
    objects::{JObject, JString, JValue},
    JNIEnv,
};
use lb_rs::model::errors::LbErr;

pub(crate) fn rstring<'local>(env: &mut JNIEnv<'local>, input: &JString<'local>) -> String {
    env.get_string(input).unwrap().to_str().unwrap().to_owned()
}

pub(crate) fn jni_string<'local>(env: &mut JNIEnv<'local>, input: String) -> JString<'local> {
    env.new_string(input).unwrap()
}

pub(crate) fn populate_err<'local>(env: &mut JNIEnv, class: &JObject<'local>, err: LbErr) {
    let j_err = env.find_class("Lnet/lockbook/Err").unwrap();

    let obj = env.alloc_object(j_err).unwrap();

    let msg = jni_string(env, err.to_string());
    env.set_field(&obj, "msg", "Ljava/lang/String", JValue::Object(&msg))
        .unwrap();

    env.set_field(class, "err", "Lnet/lockbook/Err", JValue::Object(&obj))
        .unwrap();
}
