//! Imagine, look, download, STT, and record (TTS to a file). Worker-thread
//! only (typed spawn / call `spawn_blocking`) — HTTP and `block_on` Lb.

use std::io::Cursor;
use std::time::Duration;

use lb_rs::blocking::Lb;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;
use serde_json::{Value, json};

use tracing::warn;

use super::auth;
use super::grok::{BASE_URL, MODEL};
use super::tools::{self, ClientToolOut};
use crate::workspace::WsPersistentStore;
use lb_rs::model::media_text::{self, Transcript};

const IMAGINE_MODEL: &str = "grok-imagine-image-2.0";
const LOOK_CAP: usize = 12 * 1024 * 1024;
const DOWNLOAD_CAP: usize = 32 * 1024 * 1024;

pub fn imagine(
    core: &Lb, chat_id: lb_rs::Uuid, args: &Value, cfg: &WsPersistentStore,
) -> Result<ClientToolOut, String> {
    let prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "imagine needs a prompt".to_string())?;
    let from = from_paths(args);
    let bearer = bearer(cfg)?;
    let png = if from.is_empty() {
        imagine_generate(&bearer, prompt, args)?
    } else {
        let mut refs = Vec::new();
        for p in &from {
            let (mime, bytes) = load_image(core, chat_id, p)?;
            refs.push(format!("data:{mime};base64,{}", b64(&bytes)));
        }
        imagine_edit(&bearer, prompt, &refs, args)?
    };
    let path = write_asset(
        core,
        chat_id,
        args.get("path").and_then(Value::as_str),
        "imagine",
        "png",
        &png,
    )?;
    Ok(ClientToolOut::text(format!("wrote {path}. Embed with ![]({path}) or open it with tabs.")))
}

pub fn look(
    core: &Lb, chat_id: lb_rs::Uuid, args: &Value, cfg: &WsPersistentStore, describe: bool,
) -> Result<ClientToolOut, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "look needs a path".to_string())?;
    let (mime, bytes) = load_image(core, chat_id, path)?;
    if describe {
        let bearer = bearer(cfg)?;
        let text = describe_image(&bearer, &mime, &bytes)?;
        return Ok(ClientToolOut::text(text));
    }
    Ok(ClientToolOut { text: format!("attached {path}"), image: Some((mime, bytes)) })
}

pub fn download(core: &Lb, chat_id: lb_rs::Uuid, args: &Value) -> Result<ClientToolOut, String> {
    let raw = args
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "download needs an http(s) URL".to_string())?;
    let got = fetch_url(&parse_http_url(raw)?)?;
    let ext = download_ext(&got.bytes, &got.url, got.mime.as_deref());
    let path = write_asset(
        core,
        chat_id,
        args.get("path").and_then(Value::as_str),
        &download_stem(&got.url),
        &ext,
        &got.bytes,
    )?;
    Ok(ClientToolOut::text(format!("wrote {path}")))
}

pub fn transcribe(
    core: &Lb, chat_id: lb_rs::Uuid, args: &Value, cfg: &WsPersistentStore,
) -> Result<ClientToolOut, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "transcribe needs a path".to_string())?;
    let file = tools::open_file(core, chat_id, path)?;
    if file.file_type == FileType::Folder {
        return Err(format!("{path} is a folder"));
    }
    let bytes = core
        .read_document(file.id, false)
        .map_err(|e| format!("couldn't read {path}: {e}"))?;
    if bytes.is_empty() {
        return Err(format!("{path} is empty"));
    }
    let bearer = bearer(cfg)?;
    let name = file.name.clone();
    let stt = stt(&bearer, &name, bytes.clone())?;
    if stt.text.is_empty() {
        return Ok(ClientToolOut::text("(empty transcript)"));
    }
    if media_text::looks_like_mp3(&name, &bytes) {
        match media_text::embed(&bytes, &stt) {
            Ok(stamped) if stamped != bytes => {
                if let Err(e) = core.write_document(file.id, &stamped) {
                    warn!(error = %e, path, "chat transcribe stamp");
                }
            }
            Err(e) => warn!(error = %e, path, "chat transcribe stamp"),
            Ok(_) => {}
        }
    }
    Ok(ClientToolOut::text(stt.text))
}

pub fn record(
    core: &Lb, chat_id: lb_rs::Uuid, args: &Value, cfg: &WsPersistentStore,
) -> Result<ClientToolOut, String> {
    let text = args
        .get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "record needs text".to_string())?;
    let voice = args
        .get("voice")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(tools::DEFAULT_VOICE);
    let bearer = bearer(cfg)?;
    let mp3 = tts(&bearer, text, voice)?;
    let stamped = media_text::embed(&mp3, &Transcript::from_text(text)).unwrap_or_else(|e| {
        warn!(error = %e, "chat record stamp");
        mp3
    });
    let path = write_asset(
        core,
        chat_id,
        args.get("path").and_then(Value::as_str),
        "record",
        "mp3",
        &stamped,
    )?;
    Ok(ClientToolOut::text(format!("wrote {path}")))
}

fn bearer(cfg: &WsPersistentStore) -> Result<String, String> {
    let mut tokens = auth::TokenSet::from_prefs(&cfg.grok()).ok_or("not signed in")?;
    let expires = tokens.expires_at;
    let b = auth::resolve_bearer(&mut tokens)?;
    if tokens.expires_at != expires {
        auth::save_prefs(cfg, &tokens);
    }
    Ok(b)
}

fn from_paths(args: &Value) -> Vec<String> {
    match args.get("from") {
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .take(5)
            .map(|s| s.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

fn load_image(core: &Lb, chat_id: lb_rs::Uuid, path: &str) -> Result<(String, Vec<u8>), String> {
    let file = tools::open_file(core, chat_id, path)?;
    if file.file_type == FileType::Folder {
        return Err(format!("{path} is a folder"));
    }
    let bytes = core
        .read_document(file.id, false)
        .map_err(|e| format!("couldn't read {path}: {e}"))?;
    if bytes.len() > LOOK_CAP {
        return Err(format!("{path} is too large to look at"));
    }
    vision_bytes(&bytes)
}

fn vision_bytes(bytes: &[u8]) -> Result<(String, Vec<u8>), String> {
    match image::guess_format(bytes) {
        Ok(image::ImageFormat::Jpeg) => Ok(("image/jpeg".into(), bytes.to_vec())),
        Ok(image::ImageFormat::Png) => Ok(("image/png".into(), bytes.to_vec())),
        Ok(_) => {
            let img = image::load_from_memory(bytes).map_err(|e| format!("decode image: {e}"))?;
            let mut out = Vec::new();
            img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
                .map_err(|e| format!("encode png: {e}"))?;
            Ok(("image/png".into(), out))
        }
        Err(_) => Err(not_an_image(bytes)),
    }
}

fn not_an_image(bytes: &[u8]) -> String {
    let n = bytes.len().min(80);
    let head = String::from_utf8_lossy(&bytes[..n]).to_ascii_lowercase();
    if head.contains("<svg") {
        "that's an SVG, not a raster image".into()
    } else if head.contains("<html") || head.contains("<!doctype") {
        "got a web page, not an image".into()
    } else {
        "not a jpeg or png".into()
    }
}

fn write_asset(
    core: &Lb, chat_id: lb_rs::Uuid, path: Option<&str>, stem: &str, ext: &str, bytes: &[u8],
) -> Result<String, String> {
    if let Some(path) = path.map(str::trim).filter(|s| !s.is_empty()) {
        tools::refuse_hidden(path)?;
        if path == "." || path == "/" {
            return Err("needs a file path".into());
        }
        if let Ok(existing) = core.get_by_path(path) {
            if existing.file_type == FileType::Folder {
                return Err(format!("{path} is a folder"));
            }
            core.write_document(existing.id, bytes)
                .map_err(|e| format!("couldn't write {path}: {e}"))?;
            return Ok(path.to_string());
        }
        let file = core
            .create_at_path(path)
            .map_err(|e| format!("couldn't create {path}: {e}"))?;
        core.write_document(file.id, bytes)
            .map_err(|e| format!("couldn't write {path}: {e}"))?;
        return Ok(core
            .get_path_by_id(file.id)
            .unwrap_or_else(|_| path.to_string()));
    }
    let folder = ensure_assets(core, chat_id)?;
    let name = unique_name(core, &folder, &format!("{stem}_{}", stamp()), ext);
    let file = core
        .create_file(&name, &folder.id, FileType::Document)
        .map_err(|e| format!("couldn't create {name}: {e}"))?;
    core.write_document(file.id, bytes)
        .map_err(|e| format!("couldn't write {name}: {e}"))?;
    core.get_path_by_id(file.id).map_err(|e| e.to_string())
}

fn ensure_assets(core: &Lb, chat_id: lb_rs::Uuid) -> Result<File, String> {
    let chat = core.get_file_by_id(chat_id).map_err(|e| e.to_string())?;
    let kids = core.get_children(&chat.parent).map_err(|e| e.to_string())?;
    if let Some(f) = kids
        .into_iter()
        .find(|f| f.name == "assets" && f.file_type == FileType::Folder)
    {
        return Ok(f);
    }
    core.create_file("assets", &chat.parent, FileType::Folder)
        .map_err(|e| format!("couldn't create assets/: {e}"))
}

fn unique_name(core: &Lb, folder: &File, stem: &str, ext: &str) -> String {
    let kids = core.get_children(&folder.id).unwrap_or_default();
    let file_name = |stem: &str| {
        if ext.is_empty() { stem.to_string() } else { format!("{stem}.{ext}") }
    };
    let mut name = file_name(stem);
    if !kids.iter().any(|k| k.name == name) {
        return name;
    }
    for i in 2..50 {
        name = file_name(&format!("{stem}-{i}"));
        if !kids.iter().any(|k| k.name == name) {
            return name;
        }
    }
    file_name(&format!("{stem}-{}", stamp()))
}

fn stamp() -> String {
    chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}

fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())
}

fn parse_http_url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|_| "needs an http(s) URL".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("needs an http(s) URL".into());
    }
    Ok(url)
}

fn download_stem(url: &reqwest::Url) -> String {
    let name = url.path_segments().and_then(|s| s.last()).unwrap_or("");
    let stem = name
        .rsplit_once('.')
        .map(|(s, _)| s)
        .filter(|s| !s.is_empty())
        .unwrap_or(name);
    let clean: String = stem
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(40)
        .collect();
    if clean.is_empty() { "download".into() } else { clean }
}

fn download_ext(bytes: &[u8], url: &reqwest::Url, mime: Option<&str>) -> String {
    if let Some(ext) = sniff_ext(bytes) {
        return ext.to_string();
    }
    if let Some(ext) = url_ext(url) {
        return ext;
    }
    ext_for_mime(mime.unwrap_or("")).unwrap_or("").to_string()
}

fn url_ext(url: &reqwest::Url) -> Option<String> {
    let name = url.path_segments().and_then(|s| s.last())?;
    let ext = name.rsplit_once('.')?.1;
    if ext.is_empty() || ext.len() > 8 || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let ext = ext.to_ascii_lowercase();
    Some(if ext == "jpeg" { "jpg".into() } else { ext })
}

fn ext_for_mime(mime: &str) -> Option<&'static str> {
    let mime = mime
        .split(';')
        .next()
        .unwrap_or(mime)
        .trim()
        .to_ascii_lowercase();
    Some(match mime.as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        "application/pdf" => "pdf",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/wav" | "audio/x-wav" => "wav",
        "video/mp4" => "mp4",
        "application/zip" => "zip",
        "text/markdown" => "md",
        "text/plain" => "txt",
        "text/html" => "html",
        "application/json" => "json",
        "application/xml" | "text/xml" => "xml",
        "text/csv" => "csv",
        _ => return None,
    })
}

fn sniff_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff {
        return Some("jpg");
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE" {
        return Some("wav");
    }
    if bytes.starts_with(b"%PDF") {
        return Some("pdf");
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return Some("zip");
    }
    if bytes.starts_with(b"ID3") {
        return Some("mp3");
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(160)]).to_ascii_lowercase();
    if head.contains("<svg") {
        return Some("svg");
    }
    None
}

struct Fetched {
    bytes: Vec<u8>,
    mime: Option<String>,
    url: reqwest::Url,
}

fn fetch_url(url: &reqwest::Url) -> Result<Fetched, String> {
    let resp = http()?
        .get(url.clone())
        .header("Accept", "*/*")
        .header("User-Agent", "Lockbook")
        .send()
        .map_err(|e| format!("download: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        warn!(url = %url, status = %status, "chat download http");
        return Err(format!("download HTTP {status}"));
    }
    let final_url = resp.url().clone();
    if final_url.scheme() != "http" && final_url.scheme() != "https" {
        return Err("needs an http(s) URL".into());
    }
    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    if let Some(len) = resp.content_length() {
        if len > DOWNLOAD_CAP as u64 {
            return Err("file is too large".into());
        }
    }
    let bytes = resp.bytes().map_err(|e| format!("download: {e}"))?;
    if bytes.len() > DOWNLOAD_CAP {
        return Err("file is too large".into());
    }
    if bytes.is_empty() {
        return Err("download was empty".into());
    }
    Ok(Fetched { bytes: bytes.to_vec(), mime, url: final_url })
}

fn imagine_generate(bearer: &str, prompt: &str, args: &Value) -> Result<Vec<u8>, String> {
    let mut body = json!({
        "model": IMAGINE_MODEL,
        "prompt": prompt,
        "response_format": "b64_json",
    });
    if let Some(a) = args
        .get("aspect")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        body["aspect_ratio"] = json!(a);
    }
    if let Some(q) = args
        .get("quality")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        body["quality"] = json!(q);
    }
    let v = post_json(bearer, &format!("{BASE_URL}/images/generations"), &body)?;
    image_bytes(&v)
}

fn imagine_edit(
    bearer: &str, prompt: &str, data_uris: &[String], args: &Value,
) -> Result<Vec<u8>, String> {
    let images: Vec<Value> = data_uris
        .iter()
        .map(|u| json!({ "type": "image_url", "url": u }))
        .collect();
    let mut body = json!({
        "model": IMAGINE_MODEL,
        "prompt": prompt,
        "response_format": "b64_json",
        "images": images,
    });
    if let Some(a) = args
        .get("aspect")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        body["aspect_ratio"] = json!(a);
    }
    let v = post_json(bearer, &format!("{BASE_URL}/images/edits"), &body)?;
    image_bytes(&v)
}

fn image_bytes(v: &Value) -> Result<Vec<u8>, String> {
    let first = v
        .get("data")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .ok_or_else(|| "imagine returned no image".to_string())?;
    if let Some(b64s) = first.get("b64_json").and_then(Value::as_str) {
        return decode_b64(b64s);
    }
    Err("imagine response missing b64_json".into())
}

fn describe_image(bearer: &str, mime: &str, bytes: &[u8]) -> Result<String, String> {
    let url = format!("data:{mime};base64,{}", b64(bytes));
    let body = json!({
        "model": MODEL,
        "store": false,
        "input": [{
            "role": "user",
            "content": [
                { "type": "input_image", "image_url": url, "detail": "high" },
                { "type": "input_text", "text": "Describe this image concisely for someone who cannot see it. Name what's in frame; skip filler." }
            ]
        }]
    });
    let v = post_json(bearer, &format!("{BASE_URL}/responses"), &body)?;
    if let Some(t) = v
        .get("output_text")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Ok(t.to_string());
    }
    let mut out = String::new();
    if let Some(arr) = v.get("output").and_then(Value::as_array) {
        for item in arr {
            if let Some(content) = item.get("content").and_then(Value::as_array) {
                for c in content {
                    if let Some(t) = c.get("text").and_then(Value::as_str) {
                        out.push_str(t);
                    }
                }
            }
        }
    }
    if out.is_empty() { Err("look got no description".into()) } else { Ok(out) }
}

fn stt(bearer: &str, filename: &str, bytes: Vec<u8>) -> Result<Transcript, String> {
    let file_name = if filename.is_empty() { "audio".to_string() } else { filename.to_string() };
    let part = reqwest::blocking::multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str("application/octet-stream")
        .map_err(|e| e.to_string())?;
    let form = reqwest::blocking::multipart::Form::new().part("file", part);
    let resp = http()?
        .post(format!("{BASE_URL}/stt"))
        .bearer_auth(bearer)
        .multipart(form)
        .send()
        .map_err(|e| format!("stt: {e}"))?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("stt HTTP {status}: {}", snippet(&body)));
    }
    let v: Value = serde_json::from_str(&body).map_err(|e| format!("stt json: {e}"))?;
    Ok(Transcript::from_stt(&v))
}

fn tts(bearer: &str, text: &str, voice: &str) -> Result<Vec<u8>, String> {
    let resp = http()?
        .post(format!("{BASE_URL}/tts"))
        .bearer_auth(bearer)
        .json(&json!({
            "text": text,
            "voice_id": voice,
            "language": "en",
        }))
        .send()
        .map_err(|e| format!("tts: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(format!("tts HTTP {status}: {}", snippet(&body)));
    }
    resp.bytes()
        .map(|b| b.to_vec())
        .map_err(|e| format!("tts body: {e}"))
}

fn post_json(bearer: &str, url: &str, body: &Value) -> Result<Value, String> {
    let resp = http()?
        .post(url)
        .bearer_auth(bearer)
        .json(body)
        .send()
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("HTTP {status}: {}", snippet(&text)));
    }
    serde_json::from_str(&text).map_err(|e| format!("json: {e}"))
}

fn snippet(s: &str) -> String {
    s.chars().take(240).collect()
}

pub fn b64(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (a << 16) | (b << 8) | c;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(T[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn decode_b64(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if bytes.len() % 4 != 0 {
        return Err("bad base64".into());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let a = val(chunk[0]).ok_or("bad base64")?;
        let b = val(chunk[1]).ok_or("bad base64")?;
        out.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            let c = val(chunk[2]).ok_or("bad base64")?;
            out.push((b << 4) | (c >> 2));
            if chunk[3] != b'=' {
                let d = val(chunk[3]).ok_or("bad base64")?;
                out.push((c << 6) | d);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b64_roundtrip() {
        let src = b"hello image";
        let enc = b64(src);
        assert_eq!(decode_b64(&enc).unwrap(), src);
    }

    #[test]
    fn download_url_must_be_http() {
        assert!(parse_http_url("https://example.com/a.png").is_ok());
        assert!(parse_http_url("http://example.com/a.pdf").is_ok());
        assert!(parse_http_url("file:///tmp/a.png").is_err());
        assert!(parse_http_url("ftp://example.com/a.png").is_err());
        assert!(parse_http_url("not a url").is_err());
    }

    #[test]
    fn download_stem_from_path() {
        let url = parse_http_url("https://cdn.example.com/photos/cat-photo.png?w=800").unwrap();
        assert_eq!(download_stem(&url), "cat-photo");
        let bare = parse_http_url("https://example.com/").unwrap();
        assert_eq!(download_stem(&bare), "download");
    }

    #[test]
    fn download_ext_sniffs_then_url_then_mime() {
        let png_url = parse_http_url("https://example.com/x.bin").unwrap();
        assert_eq!(
            download_ext(b"\x89PNG\r\n\x1a\nrest", &png_url, Some("application/octet-stream")),
            "png"
        );
        let pdf_url = parse_http_url("https://example.com/report.pdf").unwrap();
        assert_eq!(download_ext(b"not magic", &pdf_url, None), "pdf");
        let rs = parse_http_url("https://example.com/main.rs").unwrap();
        assert_eq!(download_ext(b"fn main() {}", &rs, None), "rs");
        let typed = parse_http_url("https://example.com/blob").unwrap();
        assert_eq!(
            download_ext(b"not magic", &typed, Some("application/pdf; charset=binary")),
            "pdf"
        );
        let none = parse_http_url("https://example.com/blob").unwrap();
        assert_eq!(download_ext(b"not magic", &none, None), "");
        assert_eq!(sniff_ext(b"%PDF-1.4"), Some("pdf"));
        assert_eq!(sniff_ext(b"ID3...."), Some("mp3"));
        assert_eq!(sniff_ext(b"<svg xmlns='http://www.w3.org/2000/svg'>"), Some("svg"));
    }

    #[test]
    fn not_an_image_messages() {
        assert!(not_an_image(b"<svg xmlns='http://www.w3.org/2000/svg'>").contains("SVG"));
        assert!(not_an_image(b"<!DOCTYPE html><html>").contains("web page"));
        assert_eq!(not_an_image(b"\x00\x01\x02"), "not a jpeg or png");
    }
}
