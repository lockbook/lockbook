use serde::Serialize;
use serde_json::json;

pub fn translate<T, E>(intermediate: Result<T, E>) -> String
where
    T: Serialize,
    E: Serialize,
{
    match intermediate {
        Ok(t) => json!({
        "tag": "Ok",
        "content": t
        }),
        Err(e) => json!({
        "tag": "Err",
        "content": e
        }),
    }
    .to_string()
}
