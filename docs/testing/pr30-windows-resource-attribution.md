# PR #30 — Windows single-file attribution acceptance

This is test-harness diagnostics only, based on main62b0229. Do not mix #29 startup changes or bounded process/I/O cleanup into this branch. Keep #29 and issue #27 open; no merge or release is authorized by this checklist.

## Local gate

```sh
python3 -B -m unittest discover -s tools -p 'test_*.py' -v
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Windows-specific Python calibration may be skipped on non-Windows but must actually execute on Windows. A skipped test is not a passing native assertion. Inspect test names/counts, not exit status alone. Core tests still use the main/#28 runtime, not the #29 candidate.

## Native calibration job

Run the independent `Windows single-file attribution calibration` job. Require the three substantive native cases to execute: two identities/two files through a separate collector; fresh-session isolation; permissive-sharing non-blocker. Preserve the native snapshot artifact, including collection failures. Validate the correct file path and PID/creation time for each known holder, remaining handle protection before explicit test cleanup, and unchanged file bytes. No external process should be stopped or unlocked.

## Full diagnostic job

The separate existing Windows extracted-CLI job must still run its unchanged three-pass plan with real .yu2 package and source-qualified candidate. If it fails, record the exact first failure, completed cycles, candidate/test-merge source and archive hashes. Do not add retries, disable parallelism or change the two-second quarantine/30-second execution settings. This run is on #30's main-derived runtime, not a rerun of #29's failed head.

For every capture inspect stages01–06 first, then07/08. A positive singleton relationship requires the same verified PID/creation pair in the baseline and the single-file query. Batch-only positives, group-only positives, PID reuse, inaccessible/truncated snapshots, session-end errors and disappeared holders remain unresolved. Missing summary plus a query-start journal is a partial/timeout, not proof of no occupancy. Stop reason and observed elapsed time must be visible.

## Review / delivery

Check exact feature and main refs, clean worktree, unchanged experiment stash, unchanged Rust/package/lockfiles/M3 data and unchanged #29 ref. Record local/native/real-engine results separately. A calibration pass is not a fix for spontaneous OS5 or historic30s timeout. Preserve the earlier failing artifact SHA256 `11fde0ca7f2e4eebc0c8a8acd02fabeb8353d2e6e0379fcdcb216b1c2fc53bfb`; never add retrospective per-file attribution it does not contain. No public candidate promotion.
