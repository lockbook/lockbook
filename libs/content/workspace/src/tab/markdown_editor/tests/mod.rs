//! Tests for the markdown editor, organized by kind.
//!
//! - [`harness`] / [`doc_gen`] — shared fixtures, not tests: a headless
//!   [`Editor`](super::Editor) driver and a structured-markdown corpus
//!   generator configured by a `Features` struct of on/off flags. Every
//!   test below consumes them.
//! - [`render_props`] / [`edit_props`] — the property **audit tables**. Each
//!   `#[test]` row calls a per-corpus runner — `run` with an explicit
//!   `Features` corpus (`all()` ⊃ `tier_a()` ⊃ `tier_b()`, …), or `run_all` / `run_simple`
//!   / `run_lists` / `run_quotes` / `run_raw` which name theirs — plus a seed
//!   count; the `_check` body states the invariant. Regressions and exact-
//!   behavior example tests live behind a banner at the bottom of each file.
//! - [`benches`] — `#[ignore]` perf harnesses.
//!
//! Why these live under `src/` rather than the crate's `tests/` directory:
//! they exercise *private* `MdRender` / `Editor` internals (fragments,
//! layout cache, `show_block`, …) that a separate integration-test crate
//! couldn't reach. The benches stay here for the same reason — they'd need
//! the same private access a `benches/` criterion crate can't have.

pub(crate) mod doc_gen;
mod harness;

mod benches;
mod edit_props;
mod folding;
mod regressions;
mod render_props;
