use super::history::{AccountHistory, DAY, History, value_at};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Count {
    pub returned: usize,
    pub eligible: usize,
}

impl Count {
    pub fn percent(self) -> Option<f64> {
        (self.eligible > 0).then(|| 100.0 * self.returned as f64 / self.eligible as f64)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cohort {
    Complete(Count),
    Partial(Count),
    Pending,
    Missing,
}
impl Cohort {
    pub fn rate(self) -> Option<f64> {
        match self {
            Self::Complete(count) | Self::Partial(count) => count.percent(),
            _ => None,
        }
    }
}

pub const RETENTION: [i64; 4] = [0, 7, 14, 30];

pub fn day(time: i64) -> i64 {
    time.div_euclid(DAY) * DAY
}

pub struct Retention {
    pub headline: [Cohort; 4],
    pub cohorts: Vec<(i64, [Cohort; 4])>,
    pub excluded: [usize; 4],
}

pub fn retention(history: &History, start: i64, end: i64) -> Retention {
    let mut signups = BTreeMap::<i64, Vec<&AccountHistory>>::new();
    for account in &history.accounts {
        if let Some(created) = account
            .created
            .filter(|created| *created >= day(start) && *created <= end)
        {
            signups.entry(day(created)).or_default().push(account);
        }
    }
    let mut result =
        Retention { headline: [Cohort::Pending; 4], cohorts: Vec::new(), excluded: [0; 4] };
    let last_day = day(end);
    let mut totals = [Count::default(); 4];
    let mut complete = [false; 4];
    for (signup_day, cohort) in signups {
        let mut points = [Cohort::Pending; 4];
        for (index, offset) in RETENTION.into_iter().enumerate() {
            let return_day = signup_day + offset * DAY;
            if return_day > end || (offset != 0 && return_day + DAY - 1 > end) {
                continue;
            }
            let cutoff = (return_day + DAY - 1).min(end);
            if cohort.iter().any(|account| !account.observed_signup)
                || !history.covered(signup_day, (signup_day + DAY - 1).min(end))
                || !history.covered(return_day, cutoff)
            {
                points[index] = Cohort::Missing;
                result.excluded[index] += 1;
                continue;
            }
            let count = Count {
                eligible: cohort.len(),
                returned: cohort
                    .iter()
                    .filter(|account| account.active(return_day, cutoff))
                    .count(),
            };
            points[index] = if return_day + DAY - 1 > end {
                Cohort::Partial(count)
            } else {
                Cohort::Complete(count)
            };
            if index > 0 {
                complete[index] = true;
                totals[index].eligible += count.eligible;
                totals[index].returned += count.returned;
            }
        }
        if signup_day == last_day {
            result.headline[0] = points[0];
        }
        result.cohorts.push((signup_day, points));
    }
    if result.headline[0] == Cohort::Pending {
        result.headline[0] = if !history.covered(last_day, end) {
            Cohort::Missing
        } else if end < last_day + DAY - 1 {
            Cohort::Partial(Count::default())
        } else {
            Cohort::Complete(Count::default())
        };
    }
    for index in 1..4 {
        result.headline[index] = if complete[index] {
            Cohort::Complete(totals[index])
        } else if result.excluded[index] > 0 {
            Cohort::Missing
        } else {
            Cohort::Pending
        };
    }
    result
}

#[derive(Clone, Copy, Default)]
pub struct SeriesPoint {
    pub time: i64,
    pub users: Option<f64>,
    pub signups: Option<f64>,
    pub active: [Option<f64>; 4],
    pub stickiness: Option<f64>,
    pub sharing: Option<f64>,
    pub premium: [Option<f64>; 4],
    pub files: Option<f64>,
    pub storage: Option<f64>,
    pub egress: Option<f64>,
}

pub fn sample_times(start: i64, end: i64) -> Vec<i64> {
    if end == start {
        return vec![end];
    }
    let count = ((end - start) / (DAY / 4)).clamp(1, 180);
    (0..=count)
        .map(|step| start + (end - start) * step / count)
        .collect()
}

pub fn overview(history: &History, start: i64, end: i64) -> Vec<SeriesPoint> {
    sample_times(start, end)
        .into_iter()
        .map(|time| {
            let mut point = SeriesPoint { time, ..Default::default() };
            let Some(seq) = history.sequence_at(time) else { return point };
            let Some(state) = history.checkpoint(seq) else { return point };
            point.users = Some(state.users as f64);
            point.sharing = Some(state.shared as f64);
            point.files = Some(state.files as f64);
            point.storage = (state.unknown_storage == 0).then_some(state.storage as f64);
            point.premium[0] = Some(state.premium.into_iter().sum::<usize>() as f64);
            for (target, count) in point.premium[1..].iter_mut().zip(state.premium) {
                *target = Some(count as f64);
            }
            if history.covered(time - 7 * DAY, time) {
                point.signups = Some(
                    history
                        .accounts
                        .iter()
                        .filter(|account| {
                            account.observed_signup
                                && account.created.is_some_and(|created| {
                                    created > time - 7 * DAY && created <= time
                                })
                        })
                        .count() as f64,
                );
            }
            for (target, days) in point.active.iter_mut().zip([1, 2, 7, 30]) {
                if history.covered(time - days * DAY, time) {
                    *target = Some(
                        history
                            .accounts
                            .iter()
                            .filter(|account| account.active(time - days * DAY + 1, time))
                            .count() as f64,
                    );
                }
            }
            if let (Some(daily), Some(monthly)) = (point.active[0], point.active[3]) {
                point.stickiness = (monthly > 0.0).then_some(daily * 100.0 / monthly);
            }
            if history.covered(time - 30 * DAY, time) {
                if let Some(previous) = history.sequence_at(time - 30 * DAY) {
                    let now = value_at(&history.server_egress, seq);
                    let before = value_at(&history.server_egress, previous);
                    point.egress = now.checked_sub(before).map(|bytes| bytes as f64);
                }
            }
            point
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn retention_counts_exact_utc_days_and_deduplicates_activity() {
        let created = DAY + DAY - 2_000;
        let history = History {
            accounts: vec![AccountHistory {
                created: Some(created),
                observed_signup: true,
                activity: BTreeSet::from([
                    created + 10_000,
                    8 * DAY,
                    8 * DAY + 10_000,
                    15 * DAY,
                    31 * DAY,
                ]),
                ..Default::default()
            }],
            anchors: vec![(1, 0), (40 * DAY, 200)],
            latest: 200,
            ..Default::default()
        };
        let report = retention(&history, DAY, 39 * DAY);
        let cohort = report.cohorts[0].1;
        assert_eq!(cohort[0].rate(), Some(0.0));
        assert_eq!(cohort[1].rate(), Some(100.0));
        assert_eq!(cohort[2].rate(), Some(100.0));
        assert_eq!(cohort[3].rate(), Some(100.0));
        assert_eq!(retention(&history, DAY, 8 * DAY).cohorts[0].1[1], Cohort::Pending);
    }

    #[test]
    fn a_gap_never_becomes_zero_retention() {
        let history = History {
            accounts: vec![AccountHistory {
                created: Some(DAY + 1),
                observed_signup: true,
                ..Default::default()
            }],
            anchors: vec![(1, 0), (32 * DAY, 32)],
            gaps: vec![(7, 10)],
            latest: 32,
            ..Default::default()
        };
        assert_eq!(retention(&history, DAY, 31 * DAY).cohorts[0].1[1], Cohort::Missing);
    }
}
