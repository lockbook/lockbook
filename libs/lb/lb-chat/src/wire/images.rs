//! Pictures made on request, by a provider with an images endpoint.

use serde_json::{Value, json};

use super::{ToolSchema, send};
use crate::provider::Provider;

/// The tool a model asks for a picture with.
pub const NAME: &str = "generate_image";

pub fn schema() -> ToolSchema {
    ToolSchema {
        name: NAME.into(),
        description: "Make a picture from a description. Answers with the path it was saved at; \
            write ![](path) in your reply to show it."
            .into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "what the picture shows" },
            },
            "required": ["prompt"],
            "additionalProperties": false,
        }),
    }
}

/// A picture of `prompt` from `model`: its file extension and its bytes.
pub async fn generate(
    client: &reqwest::Client, provider: &Provider, model: &str, prompt: &str,
) -> Result<(String, Vec<u8>), String> {
    let mut headers = Vec::new();
    if let Some(key) = &provider.api_key {
        headers.push(("authorization", format!("Bearer {key}")));
    }
    let url = format!("{}/images/generations", provider.base_url);
    let body = json!({ "model": model, "prompt": prompt, "response_format": "b64_json" });
    let answer: Value = send(client, &url, &headers, &body)
        .await?
        .json()
        .await
        .map_err(|e| format!("the picture did not arrive: {e}"))?;
    picture(&answer["data"][0])
}

/// The picture in one item of an images answer.
fn picture(item: &Value) -> Result<(String, Vec<u8>), String> {
    let data = item["b64_json"]
        .as_str()
        .ok_or("the provider sent no picture")?;
    let bytes = base64::decode(data).map_err(|_| "the picture did not decode")?;
    let ext = match item["mime_type"].as_str() {
        Some("image/jpeg") => "jpg",
        Some("image/webp") => "webp",
        _ => "png",
    };
    Ok((ext.into(), bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_comes_with_its_kind() {
        let made = json!({ "b64_json": "AAEC", "mime_type": "image/jpeg" });
        assert_eq!(picture(&made), Ok(("jpg".into(), vec![0, 1, 2])));
        assert_eq!(picture(&json!({ "b64_json": "AAEC" })).unwrap().0, "png");
        assert!(picture(&json!({ "url": "https://x" })).is_err());
    }
}
