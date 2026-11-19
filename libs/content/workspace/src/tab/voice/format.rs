use serde::{Deserialize, Serialize};

pub const MAGIC: &[u8; 8] = b"LBVOICE\0";
pub const VERSION: u32 = 1;
const HEADER_LEN: usize = 16;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Meta {
    #[serde(default)]
    pub audio_type: String,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<Transcript>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Transcript {
    pub engine: String,
    pub locale: String,
    pub took_ms: u64,
    pub segments: Vec<Segment>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Segment {
    pub words: Vec<Word>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Word {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

impl Transcript {
    pub fn words(&self) -> impl Iterator<Item = &Word> {
        self.segments.iter().flat_map(|s| s.words.iter())
    }

    pub fn text(&self) -> String {
        self.words().map(|w| w.text.as_str()).collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VoiceFile {
    pub meta: Meta,
    pub audio: Vec<u8>,
}

impl VoiceFile {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        if bytes.len() < HEADER_LEN || &bytes[..8] != MAGIC {
            return Err("This isn't a Lockbook voice file.".into());
        }
        let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if version > VERSION {
            return Err(format!(
                "This voice file is version {version}; update Lockbook to open it."
            ));
        }
        let meta_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let meta_end = HEADER_LEN
            .checked_add(meta_len)
            .filter(|&end| end <= bytes.len())
            .ok_or("This voice file is truncated.")?;
        let meta: Meta = serde_json::from_slice(&bytes[HEADER_LEN..meta_end])
            .map_err(|e| format!("This voice file's details are unreadable: {e}"))?;
        Ok(Self { meta, audio: bytes[meta_end..].to_vec() })
    }

    pub fn encode(&self) -> Vec<u8> {
        let meta = serde_json::to_vec(&self.meta).unwrap_or_default();
        let mut out = Vec::with_capacity(HEADER_LEN + meta.len() + self.audio.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(meta.len() as u32).to_le_bytes());
        out.extend_from_slice(&meta);
        out.extend_from_slice(&self.audio);
        out
    }

    pub fn has_audio(&self) -> bool {
        !self.audio.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bytes_are_an_empty_note() {
        assert_eq!(VoiceFile::decode(&[]).unwrap(), VoiceFile::default());
    }

    #[test]
    fn round_trips() {
        let file = VoiceFile {
            meta: Meta {
                audio_type: "m4a".into(),
                duration_ms: 4200,
                transcript: Some(Transcript {
                    engine: "apple-speech-transcriber".into(),
                    locale: "en_US".into(),
                    took_ms: 310,
                    segments: vec![Segment {
                        words: vec![
                            Word { text: "Hello".into(), start_ms: 0, end_ms: 400 },
                            Word { text: " world".into(), start_ms: 450, end_ms: 900 },
                        ],
                    }],
                }),
            },
            audio: vec![1, 2, 3, 4, 5],
        };
        let decoded = VoiceFile::decode(&file.encode()).unwrap();
        assert_eq!(decoded, file);
        assert_eq!(decoded.meta.transcript.unwrap().text(), "Hello world");
    }

    #[test]
    fn rejects_other_files() {
        assert!(VoiceFile::decode(b"# a note").is_err());
    }

    #[test]
    fn rejects_truncated_meta() {
        let mut bytes = VoiceFile::default().encode();
        bytes[12..16].copy_from_slice(&1000u32.to_le_bytes());
        assert!(VoiceFile::decode(&bytes).is_err());
    }
}
