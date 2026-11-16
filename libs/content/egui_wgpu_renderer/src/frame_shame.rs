use std::fmt::Write;
use std::time::Duration;

pub fn frame_budget_destroyed(
    elapsed: Duration, budget: Duration, phases: &[(&'static str, Duration)],
) -> ! {
    panic!("{}", report(elapsed, budget, phases));
}

fn report(elapsed: Duration, budget: Duration, phases: &[(&'static str, Duration)]) -> String {
    let elapsed_ms = ms(elapsed);
    let budget_ms = ms(budget);
    let overrun_ms = elapsed_ms - budget_ms;

    let mut breakdown = String::new();
    match phases.iter().max_by_key(|(_, took)| *took) {
        Some((slowest, took)) => {
            let _ = writeln!(breakdown, "Slowest phase: {slowest} ({:.2}ms)", ms(*took));
            let width = phases.iter().map(|(name, _)| name.len()).max().unwrap_or(0);
            for (name, took) in phases {
                let _ = writeln!(breakdown, "  {name:<width$}  {:>9.2}ms", ms(*took));
            }
        }
        None => breakdown.push_str("No phases were recorded.\n"),
    }

    format!(
        "\n\
         ============================================================\n\
         FRAME BUDGET DESTROYED: {elapsed_ms:.2}ms / {budget_ms:.2}ms ({overrun_ms:.2}ms over)\n\
         ============================================================\n\
         {breakdown}\
         \n\
         Whoever wrote the code that just ran should be ashamed of themselves.\n\
         Your render loop is moving so slowly the GPU filed a missing-persons report.\n\
         That frame had ONE job — finish in {budget_ms:.2}ms — and it choked like a\n\
         freshman bootcamp grad on a whiteboard interview. Geologists are studying\n\
         your event loop to model glacial retreat. The CPU was sitting there\n\
         twiddling its silicon thumbs while your single-threaded spaghetti tied\n\
         itself in knots. Somewhere a hardware engineer just retired early because\n\
         of code like this. Dropped frames don't grow on trees, you absolute walnut.\n\
         \n\
         Hand in your keyboard. Take up woodworking. The compiler deserves better.\n\
         ============================================================"
    )
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_names_the_slowest_phase() {
        let ms = Duration::from_millis;
        let report = report(
            ms(500),
            ms(400),
            &[("egui input", ms(1)), ("workspace", ms(480)), ("egui end of pass", ms(19))],
        );

        assert!(report.contains("Slowest phase: workspace (480.00ms)"), "{report}");
        assert!(report.contains("egui end of pass      19.00ms"), "{report}");
    }
}
