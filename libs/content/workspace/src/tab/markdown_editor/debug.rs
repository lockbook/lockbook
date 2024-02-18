use std::time::Instant;

pub struct DebugInfo {
    pub draw_enabled: bool,

    pub frame_count: usize,
    pub frame_start: Instant,
}

impl Default for DebugInfo {
    fn default() -> Self {
        Self { frame_count: 0, draw_enabled: false, frame_start: Instant::now() }
    }
}

impl DebugInfo {
    pub fn frame_start(&mut self) {
        self.frame_start = Instant::now();
        self.frame_count += 1;
    }

    pub fn ms_elapsed(&self) -> u128 {
        (Instant::now() - self.frame_start).as_millis()
    }
}
