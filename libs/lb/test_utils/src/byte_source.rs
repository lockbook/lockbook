//! Byte-stream test case generation.
//!
//! A `ByteSource` wraps a fixed buffer and exposes draw/bias methods that
//! generators consume to make choices. When the buffer is exhausted, draws
//! return their lowest-entropy result (0 for `draw`, weights[0] for `bias`)
//! — the "boring" path. Generators MUST order options boring → exotic so
//! shorter buffers yield simpler test cases.

pub struct ByteSource<'a> {
    buf: &'a [u8],
    cursor: usize,
}

impl<'a> ByteSource<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, cursor: 0 }
    }

    /// Returns a value in `[0, n)`. Returns 0 when the buffer is exhausted.
    pub fn draw(&mut self, n: usize) -> usize {
        assert!(n > 0, "draw(0) is meaningless");
        if self.cursor >= self.buf.len() {
            return 0;
        }
        let b = self.buf[self.cursor] as usize;
        self.cursor += 1;
        b % n
    }

    /// Weighted choice. Returns the index of the chosen weight.
    /// On exhaustion returns 0; place the boring option first.
    pub fn bias(&mut self, weights: &[u32]) -> usize {
        assert!(!weights.is_empty(), "bias(&[]) is meaningless");
        let total: u64 = weights.iter().map(|&w| w as u64).sum();
        if total == 0 {
            return 0;
        }
        if self.cursor >= self.buf.len() {
            return 0;
        }
        let b = self.buf[self.cursor] as u64;
        self.cursor += 1;
        let mut t = (b * total) / 256;
        if t >= total {
            t = total - 1;
        }
        let mut acc: u64 = 0;
        for (i, &w) in weights.iter().enumerate() {
            acc += w as u64;
            if t < acc {
                return i;
            }
        }
        weights.len() - 1
    }
}
