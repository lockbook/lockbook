//! Shared seed loop for the byte-stream property tests. Runs `check`
//! against `seeds` seeds; on the first failure it delta-debugs the
//! buffer and panics with a `repro`-formatted reproducer.
//!
//! `PROP_SLICE=n` moves every property to its nth range of seeds,
//! `n*seeds..(n+1)*seeds`, so a repeat run on unchanged code covers new
//! seeds. Unset is slice 0.
//!
//! On CI a slow run stops at [`budget`] rather than failing: a property
//! test should report a *broken invariant*, not a slow machine. Coverage
//! degrades (logged), the test stays green. A local run does every seed.
//! Per-property seed counts live at the call sites so the property table
//! reads as `(property, seeds)`.

use std::io::Write;
use std::time::{Duration, Instant};

use rand::{Rng, SeedableRng, rngs::StdRng};

use crate::shrink::shrink;

/// Wallclock ceiling for one property test on CI, where test time is the
/// budget. Generous — a safety stop for a slow runner, not a per-property budget.
const BUDGET: Duration = Duration::from_secs(120);

/// [`BUDGET`] on CI; a local run has none.
pub fn budget() -> Option<Duration> {
    std::env::var_os("CI").is_some().then_some(BUDGET)
}

/// Runs `check` over `seeds` random buffers of `buf_len` bytes. `repro`
/// renders a shrunken failing buffer into a human-readable reproducer
/// (e.g. the regenerated markdown) for the panic message.
pub fn run<E, C, R>(seeds: u64, buf_len: usize, check: C, repro: R)
where
    E: std::fmt::Display,
    C: Fn(&[u8]) -> Result<(), E>,
    R: Fn(&[u8]) -> String,
{
    run_until(budget().map(|budget| Instant::now() + budget), seeds, buf_len, check, repro);
}

/// [`run`] stopping at `deadline`, for a property that splits one budget among several runs.
pub fn run_until<E, C, R>(deadline: Option<Instant>, seeds: u64, buf_len: usize, check: C, repro: R)
where
    E: std::fmt::Display,
    C: Fn(&[u8]) -> Result<(), E>,
    R: Fn(&[u8]) -> String,
{
    let slice = slice(std::env::var(SLICE));
    let first = slice
        .checked_mul(seeds)
        .unwrap_or_else(|| panic!("{SLICE}={slice} overflows"));
    let last = first
        .checked_add(seeds)
        .unwrap_or_else(|| panic!("{SLICE}={slice} overflows"));
    let at = if slice == 0 { String::new() } else { format!(" ({SLICE}={slice})") };
    for seed in first..last {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            // Write straight to fd 2: libtest captures (hides) a passing
            // test's `eprintln!`, so a budget-truncated but green run would
            // otherwise silently report reduced coverage as a clean pass.
            // `thread::current().name()` is the test's name under libtest.
            let thread = std::thread::current();
            let name = thread.name().unwrap_or("?");
            let _ = writeln!(
                std::io::stderr(),
                "⚠ prop {name}: stopped after {}/{seeds} seeds{at} (hit its budget) \
                 — coverage reduced this run",
                seed - first,
            );
            break;
        }
        let mut rng = StdRng::seed_from_u64(seed);
        let mut buf = vec![0u8; buf_len];
        rng.fill(&mut buf[..]);
        if let Err(reason) = check(&buf) {
            // Shrink past the budget if needed: a failing test fails
            // regardless, and a small reproducer is worth the extra time.
            // A second deadline still bounds pathological shrink loops.
            let shrink_deadline = Instant::now() + BUDGET;
            let shrunk = shrink(buf, |b| Instant::now() < shrink_deadline && check(b).is_err());
            panic!(
                "seed {seed}{at} {reason}\nshrunk ({} bytes): {shrunk:?}\n{}",
                shrunk.len(),
                repro(&shrunk),
            );
        }
    }
}

/// The variable naming the slice of seeds a run covers.
const SLICE: &str = "PROP_SLICE";

/// The slice `var` names: 0 when unset, and a failed test when it is not a number.
fn slice(var: Result<String, std::env::VarError>) -> u64 {
    match var {
        Err(std::env::VarError::NotPresent) => 0,
        Err(err) => panic!("{SLICE}: {err}"),
        Ok(value) => value
            .parse()
            .unwrap_or_else(|_| panic!("{SLICE}={value:?} is not a slice number")),
    }
}

#[cfg(test)]
mod tests {
    use std::env::VarError;

    use super::slice;

    #[test]
    fn unset_is_slice_zero() {
        assert_eq!(slice(Err(VarError::NotPresent)), 0);
        assert_eq!(slice(Ok("0".into())), 0);
        assert_eq!(slice(Ok("7".into())), 7);
    }

    #[test]
    #[should_panic(expected = "is not a slice number")]
    fn a_word_is_not_a_slice() {
        slice(Ok("next".into()));
    }
}
