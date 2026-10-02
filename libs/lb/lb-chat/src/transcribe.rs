//! Speech in a recording, written down once into a note beside it. Any of
//! the user's providers with a transcription endpoint does it, whichever
//! model the chat itself is using.

use lb_rs::blocking::Lb;
use reqwest::multipart::{Form, Part};
use serde_json::Value;

use crate::provider::{Provider, host};

/// What a recording's transcript is named: the recording's name and this.
pub const SIDECAR: &str = ".transcript.md";
/// The largest recording that is sent to be transcribed.
const RECORDING_MAX: usize = 25 * 1024 * 1024;
/// Where a real request has written a recording down: host and model.
const TRANSCRIBES: &[(&str, &str)] =
    &[("api.groq.com", "whisper-large-v3-turbo"), ("api.openai.com", "gpt-4o-mini-transcribe")];

/// Whether `path` names a recording: something to be listened to.
pub fn recorded(path: &str) -> bool {
    let name = path.to_lowercase();
    [".mp3", ".m4a", ".wav", ".ogg", ".flac", ".aac", ".webm"]
        .iter()
        .any(|ext| name.ends_with(ext))
}

/// The first of the user's providers, by name, that transcribes, with the
/// model it does it with.
pub fn transcriber(lb: &Lb) -> Option<(Provider, &'static str)> {
    let folder = lb.get_by_path("/.agent/providers").ok()?;
    let mut files = lb.get_children(&folder.id).ok()?;
    files.sort_by(|a, b| a.name.cmp(&b.name));
    files.iter().find_map(|file| {
        let name = file.name.strip_suffix(".json")?;
        let provider = Provider::load(lb, name, "").ok()?;
        let at = host(&provider.base_url);
        let (_, model) = TRANSCRIBES.iter().find(|(host, _)| *host == at)?;
        provider.api_key.is_some().then_some((provider, *model))
    })
}

/// What is said in the recording `bytes`, named `name`.
pub fn transcribe(
    provider: &Provider, model: &str, name: &str, bytes: Vec<u8>,
) -> Result<String, String> {
    if bytes.len() > RECORDING_MAX {
        let mb = bytes.len() / (1024 * 1024);
        return Err(format!("{name} is {mb} MB, too long to transcribe"));
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async {
        let form = Form::new()
            .text("model", model.to_string())
            .part("file", Part::bytes(bytes).file_name(name.to_string()));
        let mut request = reqwest::Client::new()
            .post(format!("{}/audio/transcriptions", provider.base_url))
            .multipart(form);
        if let Some(key) = &provider.api_key {
            request = request.bearer_auth(key);
        }
        let resp = request.send().await.map_err(crate::wire::unsent)?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(crate::wire::explain(status, &body));
        }
        let answer: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
        let text = answer["text"]
            .as_str()
            .ok_or("the provider sent no transcript")?;
        Ok(text.trim().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;
    use crate::provider::Kind;

    #[test]
    fn a_recording_is_written_down_by_who_can() {
        const ANSWER: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n\
            {\"text\":\" The code word is pelican. \"}";
        let (base_url, sent) = mock::serve_capturing(ANSWER);
        let provider = Provider {
            name: "p".into(),
            display_name: None,
            needs_key: false,
            kind: Kind::OpenAi,
            base_url,
            api_key: Some("k".into()),
            model: String::new(),
            effort: None,
        };
        let said = transcribe(&provider, "m", "memo.m4a", vec![1, 2, 3]).unwrap();
        assert_eq!(said, "The code word is pelican.");
        assert!(sent.recv().unwrap().contains("filename=\"memo.m4a\""));

        assert!(recorded("/a/Memo.M4A") && !recorded("/a/memo.md"));
        let long = vec![0; RECORDING_MAX + 1];
        assert!(
            transcribe(&provider, "m", "long.wav", long)
                .unwrap_err()
                .contains("too long")
        );
    }
}
