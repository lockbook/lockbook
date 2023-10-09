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

#[cfg(test)]
mod unit_tests {
    use crate::json_interface::translate;
    use lockbook_core::UnexpectedError;

    #[test]
    fn sanity_check() {
        let a: Result<(), UnexpectedError> = Err(UnexpectedError::new("test".to_string()));
        println!("{}", translate(a));
    }
}
