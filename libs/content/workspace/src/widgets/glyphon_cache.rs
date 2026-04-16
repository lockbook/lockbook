use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlyphonCacheKey {
    pub spans: Vec<GlyphonCacheSpan>,
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

impl GlyphonCacheKey {
    pub fn single(
        text: impl Into<String>, family: GlyphonFontFamily, bold: bool, italic: bool,
        color: Option<[u8; 4]>, font_size_bits: u32, line_height_bits: u32, width_bits: u32,
    ) -> Self {
        Self {
            spans: vec![GlyphonCacheSpan {
                text: text.into(),
                family,
                bold,
                italic,
                color,
            }],
            font_size_bits,
            line_height_bits,
            width_bits,
        }
    }
}

pub struct GlyphonCache {
    current: HashMap<GlyphonCacheKey, Arc<RwLock<glyphon::Buffer>>>,
    previous: HashMap<GlyphonCacheKey, Arc<RwLock<glyphon::Buffer>>>,
    began_this_frame: bool,
    hits: usize,
    misses: usize,
}

impl Default for GlyphonCache {
    fn default() -> Self {
        Self::new()
    }
}

impl GlyphonCache {
    pub fn new() -> Self {
        Self {
            current: HashMap::new(),
            previous: HashMap::new(),
            began_this_frame: false,
            hits: 0,
            misses: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        if self.began_this_frame {
            return;
        }
        self.began_this_frame = true;
        #[cfg(debug_assertions)]
        {
            self.hits = 0;
            self.misses = 0;
        }
        self.previous = std::mem::take(&mut self.current);
    }

    pub fn end_frame(&mut self) {
        self.began_this_frame = false;
    }

    pub fn stats(&self) -> (usize, usize, usize) {
        (self.hits, self.misses, self.previous.len())
    }

    pub fn get_or_shape(
        &mut self, key: GlyphonCacheKey, shape_fn: impl FnOnce() -> glyphon::Buffer,
    ) -> Arc<RwLock<glyphon::Buffer>> {
        if let Some(buf) = self.current.get(&key) {
            self.hits += 1;
            return buf.clone();
        }
        if let Some(buf) = self.previous.remove(&key) {
            self.hits += 1;
            self.current.insert(key, buf.clone());
            return buf;
        }
        self.misses += 1;
        let buf = Arc::new(RwLock::new(shape_fn()));
        self.current.insert(key, buf.clone());
        buf
    }
}
