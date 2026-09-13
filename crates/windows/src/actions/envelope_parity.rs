//! ActionResult / classifier error wire-shape parity pins (macOS structure).
//!
//! Hot-path cost baseline: A19-8 captures under
//! `probes/windows/19-semantic-actions/captures/semantic-cost-{devbox,ci}.json`.
//! Shared target pre-read baseline remains A18-7
//! (`probes/windows/18-actionability/captures/actionability-cost-*.json`).

#[cfg(test)]
#[path = "envelope_parity_tests.rs"]
mod tests;
