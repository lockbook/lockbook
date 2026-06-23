use serde::Serialize;
use serde::de::DeserializeOwned;

/// Header used by both client and server to negotiate body encoding.
pub const WIRE_FORMAT_HEADER: &str = "X-Lockbook-Wire-Format";

// https://github.com/lockbook/lockbook/issues/4768
pub const OS_HEADER: &str = "X-Lockbook-OS";

pub const CLIENT_HEADER: &str = "X-Lockbook-Client";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireFormat {
    Json,
    Bincode,
}

impl WireFormat {
    pub const CLIENT_DEFAULT: Self = WireFormat::Bincode;

    pub fn as_str(self) -> &'static str {
        match self {
            WireFormat::Json => "json",
            WireFormat::Bincode => "bincode",
        }
    }

    pub fn from_header(value: Option<&str>) -> Self {
        match value.map(|v| v.trim()) {
            Some(v) if v.eq_ignore_ascii_case("bincode") => WireFormat::Bincode,
            _ => WireFormat::Json,
        }
    }

    pub fn serialize<T: Serialize + ?Sized>(self, value: &T) -> Result<Vec<u8>, WireError> {
        match self {
            WireFormat::Json => {
                serde_json::to_vec(value).map_err(|e| WireError::Serialize(e.to_string()))
            }
            WireFormat::Bincode => {
                bincode::serialize(value).map_err(|e| WireError::Serialize(e.to_string()))
            }
        }
    }

    pub fn deserialize<T: DeserializeOwned>(self, bytes: &[u8]) -> Result<T, WireError> {
        match self {
            WireFormat::Json => {
                serde_json::from_slice(bytes).map_err(|e| WireError::Deserialize(e.to_string()))
            }
            WireFormat::Bincode => {
                bincode::deserialize(bytes).map_err(|e| WireError::Deserialize(e.to_string()))
            }
        }
    }
}

#[derive(Debug)]
pub enum WireError {
    Serialize(String),
    Deserialize(String),
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireError::Serialize(e) => write!(f, "serialize: {e}"),
            WireError::Deserialize(e) => write!(f, "deserialize: {e}"),
        }
    }
}

impl std::error::Error for WireError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_header_handles_common_inputs() {
        assert_eq!(WireFormat::from_header(None), WireFormat::Json);
        assert_eq!(WireFormat::from_header(Some("")), WireFormat::Json);
        assert_eq!(WireFormat::from_header(Some("json")), WireFormat::Json);
        assert_eq!(WireFormat::from_header(Some("bincode")), WireFormat::Bincode);
        assert_eq!(WireFormat::from_header(Some("BINCODE")), WireFormat::Bincode);
        assert_eq!(WireFormat::from_header(Some(" bincode ")), WireFormat::Bincode);
        // Unknown values fall back to JSON.
        assert_eq!(WireFormat::from_header(Some("msgpack")), WireFormat::Json);
    }

    #[test]
    fn roundtrip_byte_heavy_payload() {
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
        struct Doc {
            #[serde(with = "serde_bytes")]
            bytes: Vec<u8>,
        }

        let doc = Doc { bytes: vec![0u8, 1, 2, 200, 255] };

        for fmt in [WireFormat::Json, WireFormat::Bincode] {
            let encoded = fmt.serialize(&doc).unwrap();
            let decoded: Doc = fmt.deserialize(&encoded).unwrap();
            assert_eq!(doc, decoded, "{fmt:?} roundtrip mismatch");
        }
    }

    #[test]
    fn bincode_is_more_compact_for_bytes_than_json() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct Doc {
            #[serde(with = "serde_bytes")]
            bytes: Vec<u8>,
        }

        let doc = Doc { bytes: (0u8..=255).collect() };
        let json_len = WireFormat::Json.serialize(&doc).unwrap().len();
        let bincode_len = WireFormat::Bincode.serialize(&doc).unwrap().len();
        assert!(
            bincode_len * 3 < json_len,
            "bincode={bincode_len} json={json_len} — bincode should be much smaller"
        );
    }
}
