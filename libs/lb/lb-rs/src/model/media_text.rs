//! In-file captions (PNG/JPEG) and timed transcripts (MP3). Search extracts
//! the same bytes the chat tools write.

use std::io::Cursor;

use id3::frame::{Comment, EncapsulatedObject, ExtendedText};
use id3::{Tag, TagLike, Version};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::chat::{Buffer, ItemKind};

const JSON_KEY: &str = "Transcript";
const VTT_DESC: &str = "Transcript";
const VTT_NAME: &str = "transcript.vtt";
const VTT_MIME: &str = "text/vtt";
const COMM_DESC: &str = "Transcript";
const CUE_PAUSE: f64 = 0.5;
const CUE_WORDS: usize = 12;
const PNG_SIG: &[u8] = &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const DESC_KEY: &str = "Description";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Transcript {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<Word>,
}

impl Transcript {
    pub fn from_stt(v: &Value) -> Self {
        let text = v
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let language = v
            .get("language")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let duration = v.get("duration").and_then(Value::as_f64);
        let words = v
            .get("words")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|w| {
                        let text = w.get("text").and_then(Value::as_str)?.to_string();
                        let start = w.get("start").and_then(Value::as_f64)?;
                        let end = w.get("end").and_then(Value::as_f64)?;
                        let speaker = w.get("speaker").and_then(Value::as_i64);
                        Some(Word { text, start, end, speaker })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self { text, language, duration, words }
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        Self { text: text.into(), language: Some("en".into()), duration: None, words: Vec::new() }
    }
}

pub fn file_ext(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or("")
}

pub fn indexable(name: &str) -> bool {
    matches!(
        file_ext(name).to_ascii_lowercase().as_str(),
        "md" | "txt" | "chat" | "png" | "jpg" | "jpeg" | "mp3"
    )
}

/// Plain text search indexes for notes, chats, image captions, and audio transcripts.
pub fn extract_index_text(name: &str, bytes: &[u8]) -> Option<String> {
    match file_ext(name).to_ascii_lowercase().as_str() {
        "md" | "txt" => String::from_utf8(bytes.to_vec())
            .ok()
            .filter(|s| !s.trim().is_empty()),
        "chat" => chat_index_text(bytes),
        "png" | "jpg" | "jpeg" => get_caption(name, bytes).filter(|s| !s.trim().is_empty()),
        "mp3" => extract(bytes)
            .map(|t| t.text)
            .filter(|s| !s.trim().is_empty()),
        _ => None,
    }
}

fn chat_index_text(bytes: &[u8]) -> Option<String> {
    let t = Buffer::new(bytes).transcript();
    let mut parts = Vec::new();
    for item in &t.items {
        if matches!(item.kind, ItemKind::User | ItemKind::Assistant) && item.has_text() {
            parts.push(item.text());
        }
    }
    if parts.is_empty() { None } else { Some(parts.join("\n")) }
}

pub fn looks_like_mp3(name: &str, bytes: &[u8]) -> bool {
    if file_ext(name).eq_ignore_ascii_case("mp3") {
        return true;
    }
    if bytes.starts_with(b"RIFF") || bytes.starts_with(b"OggS") || bytes.starts_with(b"fLaC") {
        return false;
    }
    bytes.starts_with(b"ID3") || (bytes.len() >= 2 && bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0)
}

pub fn looks_like_png(name: &str, bytes: &[u8]) -> bool {
    file_ext(name).eq_ignore_ascii_case("png") || bytes.starts_with(PNG_SIG)
}

pub fn looks_like_jpeg(name: &str, bytes: &[u8]) -> bool {
    let ext = file_ext(name).to_ascii_lowercase();
    ext == "jpg" || ext == "jpeg" || (bytes.len() >= 2 && bytes[0] == 0xff && bytes[1] == 0xd8)
}

/// Stamp transcript into MP3 ID3: COMM + TXXX JSON + GEOB WebVTT.
/// Empty text strips our frames.
pub fn embed(mp3: &[u8], t: &Transcript) -> Result<Vec<u8>, String> {
    let mut tag = Tag::read_from2(Cursor::new(mp3)).unwrap_or_else(|_| Tag::new());
    tag.remove_comment(Some(COMM_DESC), None);
    tag.remove_extended_text(Some(JSON_KEY), None);
    tag.remove_encapsulated_object(Some(VTT_DESC), Some(VTT_MIME), Some(VTT_NAME), None);
    if !t.text.is_empty() {
        tag.add_frame(Comment {
            lang: "eng".into(),
            description: COMM_DESC.into(),
            text: t.text.clone(),
        });
        let json = serde_json::to_string(t).map_err(|e| e.to_string())?;
        tag.add_frame(ExtendedText { description: JSON_KEY.into(), value: json });
        if !t.words.is_empty() {
            tag.add_frame(EncapsulatedObject {
                description: VTT_DESC.into(),
                mime_type: VTT_MIME.into(),
                filename: VTT_NAME.into(),
                data: words_to_vtt(&t.words).into_bytes(),
            });
        }
    }
    let audio = strip_id3(mp3);
    let mut out = Vec::with_capacity(audio.len() + 1024);
    tag.write_to(&mut out, Version::Id3v24)
        .map_err(|e| e.to_string())?;
    out.extend_from_slice(audio);
    Ok(out)
}

pub fn extract(mp3: &[u8]) -> Option<Transcript> {
    let tag = Tag::read_from2(Cursor::new(mp3)).ok()?;
    if let Some(ext) = tag.extended_texts().find(|t| t.description == JSON_KEY) {
        if let Ok(t) = serde_json::from_str::<Transcript>(&ext.value) {
            if !t.text.is_empty() || !t.words.is_empty() {
                return Some(t);
            }
        }
        if !ext.value.is_empty() {
            return Some(Transcript::from_text(ext.value.clone()));
        }
    }
    let text = tag
        .comments()
        .find(|c| c.description == COMM_DESC)
        .map(|c| c.text.clone())
        .filter(|t| !t.is_empty())?;
    Some(Transcript::from_text(text))
}

pub fn get_caption(name: &str, bytes: &[u8]) -> Option<String> {
    if looks_like_png(name, bytes) {
        png_get(bytes)
    } else if looks_like_jpeg(name, bytes) {
        jpeg_get(bytes)
    } else {
        None
    }
}

pub fn set_caption(name: &str, bytes: &[u8], text: &str) -> Result<Vec<u8>, String> {
    if looks_like_png(name, bytes) {
        png_set(bytes, text)
    } else if looks_like_jpeg(name, bytes) {
        jpeg_set(bytes, text)
    } else {
        Err("caption is for jpeg or png".into())
    }
}

pub fn words_to_vtt(words: &[Word]) -> String {
    let mut out = String::from("WEBVTT\n");
    if words.is_empty() {
        return out;
    }
    let mut i = 0;
    while i < words.len() {
        let start = words[i].start;
        let speaker = words[i].speaker;
        let mut end = words[i].end;
        let mut texts = vec![words[i].text.as_str()];
        i += 1;
        while i < words.len()
            && texts.len() < CUE_WORDS
            && words[i].speaker == speaker
            && words[i].start - end <= CUE_PAUSE
        {
            end = words[i].end;
            texts.push(words[i].text.as_str());
            i += 1;
        }
        let line = texts.join(" ");
        let voice = speaker.map(|s| format!("<v {s}>")).unwrap_or_default();
        out.push('\n');
        out.push_str(&format!("{} --> {}\n{voice}{line}\n", vtt_time(start), vtt_time(end)));
    }
    out
}

fn vtt_time(s: f64) -> String {
    let ms = (s.max(0.0) * 1000.0).round() as u64;
    let h = ms / 3_600_000;
    let m = (ms / 60_000) % 60;
    let sec = (ms / 1000) % 60;
    let milli = ms % 1000;
    format!("{h:02}:{m:02}:{sec:02}.{milli:03}")
}

fn strip_id3(data: &[u8]) -> &[u8] {
    let mut s = skip_id3v2(data);
    if s.len() >= 128 && s[s.len() - 128..].starts_with(b"TAG") {
        s = &s[..s.len() - 128];
    }
    s
}

fn skip_id3v2(data: &[u8]) -> &[u8] {
    if data.len() < 10 || &data[0..3] != b"ID3" {
        return data;
    }
    let size = ((data[6] as usize & 0x7f) << 21)
        | ((data[7] as usize & 0x7f) << 14)
        | ((data[8] as usize & 0x7f) << 7)
        | (data[9] as usize & 0x7f);
    let footer = if data[5] & 0x10 != 0 { 10 } else { 0 };
    let start = 10 + size + footer;
    if start <= data.len() { &data[start..] } else { data }
}

fn png_get(bytes: &[u8]) -> Option<String> {
    let mut desc_itxt = None;
    let mut desc_text = None;
    let mut comment_itxt = None;
    let mut comment_text = None;
    for (typ, data) in png_chunks(bytes)? {
        match &typ {
            b"iTXt" => {
                if let Some((kw, text)) = itxt_plain(data) {
                    if kw.eq_ignore_ascii_case(DESC_KEY) {
                        desc_itxt = Some(text.to_string());
                    } else if kw.eq_ignore_ascii_case("Comment") {
                        comment_itxt = Some(text.to_string());
                    }
                }
            }
            b"tEXt" => {
                if let Some((kw, rest)) = split0(data) {
                    let text = String::from_utf8_lossy(rest).into_owned();
                    if kw.eq_ignore_ascii_case(DESC_KEY) {
                        desc_text = Some(text);
                    } else if kw.eq_ignore_ascii_case("Comment") {
                        comment_text = Some(text);
                    }
                }
            }
            _ => {}
        }
    }
    desc_itxt
        .or(desc_text)
        .or(comment_itxt)
        .or(comment_text)
        .filter(|s| !s.is_empty())
}

fn png_set(bytes: &[u8], text: &str) -> Result<Vec<u8>, String> {
    if !bytes.starts_with(PNG_SIG) {
        return Err("not a png".into());
    }
    let chunks = png_chunks(bytes).ok_or("bad png")?;
    let mut out = PNG_SIG.to_vec();
    let mut wrote_desc = false;
    for (typ, data) in chunks {
        let skip = matches!(&typ, b"tEXt" | b"iTXt" | b"zTXt")
            && split0(data).is_some_and(|(kw, _)| kw.eq_ignore_ascii_case(DESC_KEY));
        if skip {
            continue;
        }
        if &typ == b"IEND" && !text.is_empty() && !wrote_desc {
            out.extend(png_chunk(b"iTXt", &itxt_description(text)));
            wrote_desc = true;
        }
        out.extend(png_chunk(&typ, data));
    }
    if !text.is_empty() && !wrote_desc {
        return Err("png missing IEND".into());
    }
    Ok(out)
}

fn png_chunks(bytes: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
    if !bytes.starts_with(PNG_SIG) {
        return None;
    }
    let mut i = PNG_SIG.len();
    let mut out = Vec::new();
    while i + 12 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[i..i + 4].try_into().ok()?) as usize;
        let typ: [u8; 4] = bytes[i + 4..i + 8].try_into().ok()?;
        let data_start = i + 8;
        let data_end = data_start.checked_add(len)?;
        let crc_end = data_end.checked_add(4)?;
        if crc_end > bytes.len() {
            return None;
        }
        out.push((typ, &bytes[data_start..data_end]));
        i = crc_end;
        if &typ == b"IEND" {
            break;
        }
    }
    Some(out)
}

fn png_chunk(typ: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(4 + data.len());
    body.extend_from_slice(typ);
    body.extend_from_slice(data);
    let mut out = Vec::with_capacity(12 + data.len());
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out.extend_from_slice(&png_crc(&body).to_be_bytes());
    out
}

fn png_crc(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xedb8_8320 } else { c >> 1 };
        }
    }
    !c
}

fn itxt_description(text: &str) -> Vec<u8> {
    let mut d = Vec::from(DESC_KEY.as_bytes());
    d.push(0);
    d.push(0);
    d.push(0);
    d.push(0);
    d.push(0);
    d.extend_from_slice(text.as_bytes());
    d
}

fn itxt_plain(data: &[u8]) -> Option<(&str, &str)> {
    let (kw, rest) = split0(data)?;
    if rest.len() < 2 {
        return None;
    }
    let compressed = rest[0] != 0;
    let rest = &rest[2..];
    let (_, rest) = split0(rest)?;
    let (_, textb) = split0(rest)?;
    if compressed {
        return None;
    }
    let text = std::str::from_utf8(textb).ok()?;
    Some((kw, text))
}

fn split0(data: &[u8]) -> Option<(&str, &[u8])> {
    let z = data.iter().position(|&b| b == 0)?;
    let kw = std::str::from_utf8(&data[..z]).ok()?;
    Some((kw, &data[z + 1..]))
}

fn jpeg_get(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }
    let mut i = 2usize;
    while i + 4 <= bytes.len() {
        if bytes[i] != 0xff {
            return None;
        }
        let marker = bytes[i + 1];
        if marker == 0xd9 || marker == 0xda {
            break;
        }
        if marker == 0x00 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if len < 2 || i + 2 + len > bytes.len() {
            break;
        }
        if marker == 0xfe {
            let text = std::str::from_utf8(&bytes[i + 4..i + 2 + len])
                .unwrap_or("")
                .trim_end_matches('\0')
                .to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
        i += 2 + len;
    }
    None
}

fn jpeg_set(bytes: &[u8], text: &str) -> Result<Vec<u8>, String> {
    if bytes.len() < 2 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return Err("not a jpeg".into());
    }
    let mut i = 2usize;
    while i + 4 <= bytes.len() && bytes[i] == 0xff && bytes[i + 1] == 0xfe {
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if len < 2 || i + 2 + len > bytes.len() {
            break;
        }
        i += 2 + len;
    }
    let rest = &bytes[i..];
    if text.is_empty() {
        let mut out = Vec::with_capacity(2 + rest.len());
        out.extend_from_slice(&bytes[..2]);
        out.extend_from_slice(rest);
        return Ok(out);
    }
    let payload = text.as_bytes();
    if payload.len() + 2 > 0xffff {
        return Err("caption too long".into());
    }
    let clen = (payload.len() + 2) as u16;
    let mut out = Vec::with_capacity(2 + 4 + payload.len() + rest.len());
    out.extend_from_slice(&bytes[..2]);
    out.extend_from_slice(&[0xff, 0xfe, (clen >> 8) as u8, clen as u8]);
    out.extend_from_slice(payload);
    out.extend_from_slice(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn vtt_time_format() {
        assert_eq!(vtt_time(0.24), "00:00:00.240");
        assert_eq!(vtt_time(3661.5), "01:01:01.500");
    }

    #[test]
    fn vtt_groups_on_pause() {
        let words = vec![
            Word { text: "Hi".into(), start: 0.0, end: 0.2, speaker: None },
            Word { text: "there".into(), start: 0.25, end: 0.5, speaker: None },
            Word { text: "Later".into(), start: 1.2, end: 1.5, speaker: None },
        ];
        let vtt = words_to_vtt(&words);
        assert!(vtt.starts_with("WEBVTT"));
        assert!(vtt.contains("Hi there"));
        assert!(vtt.contains("Later"));
        assert_eq!(vtt.matches("-->").count(), 2);
    }

    #[test]
    fn from_stt_words() {
        let v = json!({
            "text": "The balance is $1.",
            "language": "en",
            "duration": 3.45,
            "words": [
                {"text": "The", "start": 0.24, "end": 0.48},
                {"text": "balance", "start": 0.48, "end": 0.96, "speaker": 0}
            ]
        });
        let t = Transcript::from_stt(&v);
        assert_eq!(t.text, "The balance is $1.");
        assert_eq!(t.words.len(), 2);
        assert_eq!(t.words[1].speaker, Some(0));
    }

    #[test]
    fn mp3_roundtrip() {
        let audio = b"\xff\xfb\x90\x00payload";
        let t = Transcript {
            text: "Hello world".into(),
            language: Some("en".into()),
            duration: Some(1.2),
            words: vec![
                Word { text: "Hello".into(), start: 0.0, end: 0.4, speaker: None },
                Word { text: "world".into(), start: 0.4, end: 0.9, speaker: None },
            ],
        };
        let out = embed(audio, &t).unwrap();
        assert!(out.starts_with(b"ID3"));
        let got = extract(&out).expect("extract");
        assert_eq!(got.text, "Hello world");
        assert_eq!(got.words.len(), 2);
        assert_eq!(extract_index_text("a.mp3", &out).as_deref(), Some("Hello world"));
    }

    #[test]
    fn jpeg_caption_roundtrip() {
        let jpeg = [0xff, 0xd8, 0xff, 0xd9];
        let out = set_caption("a.jpg", &jpeg, "red bicycle").unwrap();
        assert_eq!(get_caption("a.jpg", &out).as_deref(), Some("red bicycle"));
        assert_eq!(extract_index_text("pic.jpeg", &out).as_deref(), Some("red bicycle"));
        let cleared = set_caption("a.jpg", &out, "").unwrap();
        assert!(get_caption("a.jpg", &cleared).is_none());
    }

    #[test]
    fn png_caption_roundtrip() {
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        let mut png = PNG_SIG.to_vec();
        png.extend(png_chunk(b"IHDR", &ihdr));
        png.extend(png_chunk(b"IEND", &[]));
        let out = set_caption("a.png", &png, "a red cube").unwrap();
        assert_eq!(get_caption("a.png", &out).as_deref(), Some("a red cube"));
        assert_eq!(extract_index_text("shot.png", &out).as_deref(), Some("a red cube"));
        let cleared = set_caption("a.png", &out, "").unwrap();
        assert!(get_caption("a.png", &cleared).is_none());
    }

    #[test]
    fn chat_index_user_assistant() {
        let chat = concat!(
            r#"{"id":"00000000-0000-0000-0000-00000000000a","from":"a","ts":1,"body":{"type":"open","item":"00000000-0000-0000-0000-000000000001","kind":"user"}}"#,
            "\n",
            r#"{"id":"00000000-0000-0000-0000-00000000000b","from":"a","ts":2,"body":{"type":"replace","item":"00000000-0000-0000-0000-000000000001","blocks":[{"type":"text","text":"hello cube"}]}}"#,
            "\n",
        );
        let t = extract_index_text("notes.chat", chat.as_bytes()).unwrap();
        assert!(t.contains("hello cube"));
    }
}
