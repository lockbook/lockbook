use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use smallvec::SmallVec;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlyphonCacheKey {
    pub spans: SmallVec<[GlyphonCacheSpan; 1]>,
    pub font_size_bits: u32,
    pub line_height_bits: u32,
    pub width_bits: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlyphonCacheSpan {
    pub text: String,
    pub family: GlyphonFontFamily,
    pub bold: bool,
    pub italic: bool,
    pub color: Option<[u8; 4]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GlyphonFontFamily {
    SansSerif,
    Monospace,
    Named(String),
}

pub struct GlyphonCache {
    current: HashMap<GlyphonCacheKey, Arc<RwLock<glyphon::Buffer>>>,
    previous: HashMap<GlyphonCacheKey, Arc<RwLock<glyphon::Buffer>>>,
    began_this_frame: bool,
}

impl Default for GlyphonCache {
    fn default() -> Self {
        Self::new()
    }
}

impl GlyphonCache {
    pub fn new() -> Self {
        Self { current: HashMap::new(), previous: HashMap::new(), began_this_frame: false }
    }

    pub fn begin_frame(&mut self) {
        if self.began_this_frame {
            return;
        }
        self.began_this_frame = true;
        self.previous = std::mem::take(&mut self.current);
    }

    pub fn end_frame(&mut self) {
        self.began_this_frame = false;
    }

    pub fn get_or_shape(
        &mut self, key: GlyphonCacheKey, shape_fn: impl FnOnce() -> glyphon::Buffer,
    ) -> Arc<RwLock<glyphon::Buffer>> {
        if let Some(buf) = self.current.get(&key) {
            return buf.clone();
        }
        if let Some(buf) = self.previous.remove(&key) {
            self.current.insert(key, buf.clone());
            return buf;
        }
        let buf = Arc::new(RwLock::new(shape_fn()));
        self.current.insert(key, buf.clone());
        buf
    }
}
