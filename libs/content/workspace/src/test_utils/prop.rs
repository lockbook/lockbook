//! Shared seed loop for the byte-stream property tests. Runs `check`
//! against seeds `0..seeds`; on the first failure it delta-debugs the
//! buffer and panics with a `repro`-formatted reproducer.
//!
//! A slow run stops at [`BUDGET`] rather than failing: a property test
//! should report a *broken invariant*, not a slow machine. Coverage
//! degrades (logged), the test stays green. Per-property seed counts live
//! at the call sites so the property table reads as `(property, seeds)`.

use std::io::Write;
use std::time::{Duration, Instant};

use rand::{Rng, SeedableRng, rngs::StdRng};

use super::shrink::shrink;

/// Wallclock ceiling for one property test. Generous — it's a safety stop
/// for a pathologically slow machine, not a per-property budget.
const BUDGET: Duration = Duration::from_secs(120);

/// Runs `check` over `seeds` random buffers of `buf_len` bytes. `repro`
/// renders a shrunken failing buffer into a human-readable reproducer
/// (e.g. the regenerated markdown) for the panic message.
pub fn run<E, C, R>(seeds: u64, buf_len: usize, check: C, repro: R)
where
    E: std::fmt::Display,
    C: Fn(&[u8]) -> Result<(), E>,
    R: Fn(&[u8]) -> String,
{
    let deadline = Instant::now() + BUDGET;
    for seed in 0..seeds {
        if Instant::now() >= deadline {
            // Write straight to fd 2: libtest captures (hides) a passing
            // test's `eprintln!`, so a budget-truncated but green run would
            // otherwise silently report reduced coverage as a clean pass.
            // `thread::current().name()` is the test's name under libtest.
            let thread = std::thread::current();
            let name = thread.name().unwrap_or("?");
            let _ = writeln!(
                std::io::stderr(),
                "⚠ prop {name}: stopped after {seed}/{seeds} seeds (hit {}s budget) \
                 — coverage reduced this run",
                BUDGET.as_secs(),
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
                "seed {seed} {reason}\nshrunk ({} bytes): {shrunk:?}\n{}",
                shrunk.len(),
                repro(&shrunk),
            );
        }
    }
}
