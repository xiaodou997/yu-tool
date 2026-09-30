# PR #36 — Windows remove-window sampling acceptance

This is a diagnostics-only branch from `main@2bd7b491fd66af78878043a1e5ce302d445a21dc`. Do not merge PR #34/#33 work into it.

## Local gate

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 -B -m unittest discover -s tools -p 'test_windows_remove_window_sampler.py' -v
python3 -B -m unittest discover -s tools -p 'test_*.py' -v
git diff --check
```

The three sampler calibration cases are Windows-only and may skip elsewhere; those skips are not acceptance evidence.

## Native Windows calibration

Run:

```powershell
python -B -m unittest discover -s tools -p "test_windows_remove_window_sampler.py" -v
```

All three cases must execute:

1. ~500 ms non-delete-sharing holder: more than one rename attempt, final success within the two-second test retry budget, exact holder PID + creation FILETIME present in the timeline;
2. >2 s holder: final rename failure after at least 2 s, exact holder identity present;
3. sibling-only holder: target succeeds on first attempt and the test PID is absent from target identities while the sibling remains denied.

Retain calibration receipts under `target/windows-remove-window-calibration/`. The sampler must exit normally; no control may rely on terminating/unlocking the holder.

## Rust harness integration

With remove-window sampling explicitly enabled, the existing `external_directory_occupancy_fails_closed_then_recovers_after_release` Windows test must preserve the expected original failure and also verify exactly one matching failed-remove timeline containing its own PID as an identity-confirmed exact-directory user.

The sampler must finish before the existing post-failure collector starts. Post-failure evidence and CLI output remain unchanged.

## Real lifecycle / package gate

Run Windows Lifecycle Diagnostics with its existing three repetitions and Managed ag-psd Package without changing default parallelism, the two-second quarantine retry budget, or failed-case handling. Retain every remove-window report for debug and extracted-release lifecycle tests.

For each completed ten-cycle managed lifecycle suite, expect ten remove-window reports. Report successful near-misses where quarantine attempts >1, including attempts/elapsed and any overlapping identities. A finite all-first-attempt run is valid evidence but does not prove the intermittent condition disappeared.

If a spontaneous OS5 occurs, do not rerun it. Compare its in-flight timeline, post-failure exact-directory snapshot, single-file evidence and owned-process traces using PID + creation FILETIME. Temporal overlap still does not permit a causal blocker claim without handle/share-mode evidence.

## Merge boundary

A green PR establishes only that in-flight exact-directory observation is bounded, calibrated and wired to the test harness. It does not change production removal behavior, set `root_cause_fixed=true`, close Issue #27 or authorize v0.1.
