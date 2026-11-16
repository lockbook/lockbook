//! Where a frame's time went, so a frame that overruns its budget names the phase that took it.

use std::time::{Duration, Instant};

/// Starts the phase `name` of the frame being built on `ctx`: the time until the next phase starts,
/// or the frame is checked, counts toward `name`.
pub fn frame_phase(ctx: &egui::Context, name: &'static str) {
    let now = Instant::now();
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<FramePhases>(id())
            .start(name, now)
    });
}

pub(crate) fn begin_frame(ctx: &egui::Context, name: &'static str) {
    let now = Instant::now();
    ctx.data_mut(|d| {
        let phases = d.get_temp_mut_or_default::<FramePhases>(id());
        phases.clear();
        phases.start(name, now);
    });
}

/// Ends the frame's last phase and returns each phase's total time, in the order each first started.
pub(crate) fn end_frame(ctx: &egui::Context) -> Vec<(&'static str, Duration)> {
    let now = Instant::now();
    ctx.data_mut(|d| {
        let phases = d.get_temp_mut_or_default::<FramePhases>(id());
        phases.stop(now);
        phases.totals()
    })
}

fn id() -> egui::Id {
    egui::Id::unique("egui_wgpu_renderer::frame_phases")
}

#[derive(Clone, Default)]
pub(crate) struct FramePhases {
    current: Option<(&'static str, Instant)>,
    ended: Vec<(&'static str, Duration)>,
}

impl FramePhases {
    fn clear(&mut self) {
        self.current = None;
        self.ended.clear();
    }

    fn start(&mut self, name: &'static str, now: Instant) {
        self.stop(now);
        self.current = Some((name, now));
    }

    fn stop(&mut self, now: Instant) {
        if let Some((name, start)) = self.current.take() {
            self.ended
                .push((name, now.saturating_duration_since(start)));
        }
    }

    fn totals(&self) -> Vec<(&'static str, Duration)> {
        let mut totals: Vec<(&'static str, Duration)> = Vec::new();
        for &(name, took) in &self.ended {
            match totals.iter_mut().find(|(n, _)| *n == name) {
                Some((_, total)) => *total += took,
                None => totals.push((name, took)),
            }
        }
        totals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_phase_started_twice_counts_both_stretches() {
        let t0 = Instant::now();
        let mut phases = FramePhases::default();
        phases.start("input", t0);
        phases.start("sidebar", t0 + ms(1));
        phases.start("workspace", t0 + ms(3));
        phases.start("sidebar", t0 + ms(10));
        phases.stop(t0 + ms(14));

        assert_eq!(
            phases.totals(),
            vec![("input", ms(1)), ("sidebar", ms(6)), ("workspace", ms(7))]
        );
    }

    #[test]
    fn a_new_frame_forgets_the_last() {
        let t0 = Instant::now();
        let mut phases = FramePhases::default();
        phases.start("sidebar", t0);
        phases.stop(t0 + ms(5));
        phases.clear();
        phases.start("input", t0 + ms(5));
        phases.stop(t0 + ms(6));

        assert_eq!(phases.totals(), vec![("input", ms(1))]);
    }
}
