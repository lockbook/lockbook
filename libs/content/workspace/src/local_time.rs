//! A moment as the app shows it: in local time, by how long ago it was.

use chrono::{DateTime, Datelike as _, Local};

/// `at` seen from `now`: `17:57` today, `Yesterday 17:57`, `Nov 15 17:57`
/// on another day this year, and `Nov 15 2025 17:57` in another year.
pub fn label(at: DateTime<Local>, now: DateTime<Local>) -> String {
    match (now.date_naive() - at.date_naive()).num_days() {
        0 => at.format("%H:%M").to_string(),
        1 => format!("Yesterday {}", at.format("%H:%M")),
        _ if at.year() == now.year() => at.format("%b %-d %H:%M").to_string(),
        _ => at.format("%b %-d %Y %H:%M").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;

    #[test]
    fn labels_by_how_long_ago() {
        let local = |y, mo, d, h, mi| Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap();
        let now = local(2026, 11, 15, 18, 30);
        assert_eq!(label(local(2026, 11, 15, 17, 57), now), "17:57");
        assert_eq!(label(local(2026, 11, 14, 9, 5), now), "Yesterday 09:05");
        assert_eq!(label(local(2026, 3, 2, 17, 57), now), "Mar 2 17:57");
        assert_eq!(label(local(2025, 11, 15, 17, 57), now), "Nov 15 2025 17:57");
    }
}
