# Windows remove-window occupancy sampling — PR #36

Base: `main@2bd7b491fd66af78878043a1e5ce302d445a21dc` after PR #35. This is test/diagnostic infrastructure for Issue #27's intermittent Windows engine-removal OS5. PR #34 startup changes and PR #33 distribution changes remain separate.

## Why this increment exists

PR #35 made exact-directory users observable after a failed remove. A post-merge package run then produced a near-miss: one extracted-release remove needed 52 quarantine rename attempts and about 1317 ms before succeeding inside the unchanged two-second retry window. Because the command ultimately succeeded, the existing post-failure collector never ran, so the short-lived holder disappeared without identity evidence.

PR #36 observes the same exact directory **while the test-owned `yu engine remove` command is running**. It does not alter the Engine Manager retry loop or production CLI.

## Test-only sampling model

When `YU_TEST_REMOVE_WINDOW_SAMPLING_DIR` and the existing forensic Python interpreter are explicitly present, the Rust test harness starts an owned `windows_remove_window_sampler.py` subprocess immediately before an `engine remove` invocation. The sampler:

1. validates the version directory and writes an initial exact-directory sample before signalling `ready`;
2. samples approximately every 25 ms while the remove command remains in flight, with a five-second hard harness window and at most 256 samples;
3. uses PR #35's identity-confirmed exact-directory query, retaining PID + creation FILETIME + image only when confirmed;
4. stops as soon as the command finishes, the target directory disappears, the sample/window bound is reached, or the harness requests stop;
5. never asks for delete access, never changes share modes, ACLs or privileges, and never unlocks/terminates any observed process.

The Rust harness waits only for the sampler's `ready` handshake before launching the CLI. This is diagnostic observer effect and is not production behavior. On completion it stops/reaps only that owned sampler process and combines the sampler report with the original remove result. The original CLI exit code/stdout/stderr are unchanged and existing post-failure occupancy capture still runs afterwards on failure.

## Report

Each remove receives a unique test-owned directory containing:

```text
remove-window-.../
  ready
  stop
  sampling/
    samples.jsonl
    sampling-report.json
  report.json
```

`report.json` includes the original remove success/failure envelope, quarantine `attempts` / `elapsed_ms`, sampler exit status, and the aggregated identity timeline. Each identity records `first_seen_ms`, `last_seen_ms`, sample count, PID, creation FILETIME and image.

Every report retains:

```text
rename_blocker_proven = false
root_cause_fixed = false
public_release_ready = false
```

A temporal overlap is stronger evidence than a post-command sighting, but still does not expose the exact kernel handle/share mode or prove that the observed process caused a specific rename denial.

## Required calibration

Three real-Windows controls run before the lifecycle probes:

- **Transient holder**: a non-delete-sharing exact-directory handle is released at roughly 500 ms. Rename retry must first fail, eventually succeed within two seconds, and the sampler must retain the holder's PID + creation time before success.
- **Persistent holder**: the same type of handle remains beyond two seconds. Rename retry must remain failed while the sampler retains that identity.
- **Sibling holder**: only `version-other` is held. Target rename succeeds immediately and the holder must not appear in the target timeline.

The existing held-directory Rust lifecycle control also requires the enabled remove-window report to contain its test PID before the expected failed remove returns.

## Real lifecycle use

Windows Diagnostics and the Windows Managed Package debug/release tests explicitly enable sampling for their test harness. Successful near-misses are now retained rather than disappearing because no error occurred. `run_lifecycle_probe.py` records the count of completed remove-window reports per suite.

Sampling can perturb process scheduling and filesystem timing. Therefore a sampled success/failure cannot be treated as identical to an uninstrumented run. The existing package/lifecycle gates remain separate evidence, no failed run is retried, and historical failures remain immutable.

## Scope boundary

No `crates/yu-engine-manager` production code changes, no quarantine delay/retry changes, no test serialization, no system-wide handle enumeration, no external process control, and no public runtime dependency are introduced. This PR can improve attribution of B; it cannot by itself declare B fixed or close Issue #27.

See [acceptance](../testing/pr36-windows-remove-window-sampling.md) and the calibrated [exact-directory query](windows-directory-attribution.md).
