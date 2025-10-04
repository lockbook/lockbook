use std::collections::HashMap;

use google_androidpublisher3::chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthReport {
    monthly_agg: HashMap<YearMonth, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct YearMonth {
    pub year: i32,
    pub month: u32,
}

impl YearMonth {
    fn current() -> Self {
        let now = Local::now();
        Self { year: now.year(), month: now.month() }
    }
}

impl BandwidthReport {
    pub fn current_bandwidth(&self) -> usize {
        self.monthly_agg
            .get(&YearMonth::current())
            .copied()
            .unwrap_or_default()
    }

    pub fn increase_by(&mut self, inc: usize) {
        let now = YearMonth::current();
        match self.monthly_agg.get_mut(&YearMonth::current()) {
            Some(new) => *new = *new + inc,
            None => {
                self.monthly_agg.insert(now, inc);
            }
        }
    }
}
