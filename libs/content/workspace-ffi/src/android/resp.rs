use lb_external_interface::lb_rs::Uuid;
use serde::Serialize;
use workspace_rs::output::WsOutput;

#[derive(Serialize, Default)]
pub struct IntegrationOutput {
    pub workspace_resp: FfiWorkspaceResp,
    pub redraw_in: u128,
    pub copied_text: String,
    pub url_opened: String,
}

#[derive(Serialize, Default)]
pub struct FfiWorkspaceResp {
    selected_file: Uuid,
    doc_created: Uuid,

    msg: String,
    syncing: bool,
    refresh_files: bool,

    new_folder_btn_pressed: bool,
}

impl From<WsOutput> for FfiWorkspaceResp {
    fn from(value: WsOutput) -> Self {
        Self {
            selected_file: value.selected_file.unwrap_or_default(),
            msg: value.status.message,
            syncing: value.status.syncing,
            refresh_files: value.sync_done.is_some()
                || value.file_renamed.is_some()
                || value.file_created.is_some(),
            doc_created: match value.file_created {
                Some(Ok(f)) => {
                    if f.is_document() {
                        f.id.into()
                    } else {
                        Uuid::nil().into()
                    }
                }
                _ => Uuid::nil().into(),
            },
            new_folder_btn_pressed: value.new_folder_clicked,
        }
    }
}

// impl From<WsOutput> for FfiWorkspaceResp {
//     fn from(value: WsOutput) -> Self {
//         Self {
//             selected_file: to_jstring_serialized(value.selected_file.unwrap_or_default(), env),
//             msg: to_jstring(value.status.message.unwrap_or_default(), env),
//             syncing: to_jboolean(value.status.syncing),
//             refresh_files: to_jboolean(
//                 value.sync_done.is_some()
//                     || value.file_renamed.is_some()
//                     || value.file_created.is_some(),
//             ),
//             doc_created: {
//                 let id = match value.file_created {
//                     Some(Ok(f)) => {
//                         if f.is_document() {
//                             f.id.into()
//                         } else {
//                             Uuid::nil().into()
//                         }
//                     }
//                     _ => Uuid::nil().into(),
//                 };

//                 to_jstring(id, env)
//             },
//             new_folder_btn_pressed: to_jboolean(value.new_folder_clicked),
//         }
//     }
// }

// pub fn to_jstring(env: JNIEnv, v: &str) -> jstring {
//     env.new_string(v)
//         .expect("Couldn't create JString from rust string!")
//         .into_raw()
// }

// pub fn to_jstring_serialized<T: Serialize>(env: JNIEnv, v: &T) -> jstring {
//     let serialized = serde_json::to_string(v).expect("Couldn't serialize value to JSON string!");

//     env.new_string(serialized)
//         .expect("Couldn't create JString from serialized data!")
//         .into_raw()
// }

// pub fn to_jboolean(v: bool) -> jboolean {
//     if v {
//         1
//     } else {
//         0
//     }
// }
