# PR #28 — Explicit process cleanup acceptance

Scope: explicit cleanup outcome propagation only. Run against the exact final feature commit and record actual PR test-merge checkout SHA separately. Do not label this checklist a success receipt before running it.

## Ordinary gate

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
python -B -m unittest discover -s tools -p 'test_*.py' -v
```

Run `cargo test --locked -p yu-runtime-psd --lib` and inspect actual test execution. Controlled cases use a subprocess of the unit-test binary plus a real owned thread panic. They must preserve primary errors, surface cleanup failure, avoid copying panic payloads, drain workers and cache completed outcomes. The fixture-child test returns immediately in ordinary discovery; its selected child invocation waits for request EOF.

The staged-PNG fault case must verify a valid RGBA8 PNG was created before the simulated failed settlement, then no final publication, no source change, no staging residue, and no overwrite of a racing writer. A separate success control still publishes a valid PNG. No environment fault switch or global mutation is permitted.

## Real engine and Windows trace

Use the existing Managed ag-psd Package workflow on all three targets, including extracted release executables and ten lifecycle cycles per suite. Use the existing independent Windows Lifecycle Diagnostics workflow for its predeclared three passes: five native cases and the three real-package suites per pass, including thirty complete lifecycle cycles if all passes succeed. Do not rerun a failed case until green or suppress the first-failure artifact.

When trace is enabled, the controlled quiet-descendant case requires `cleanup_succeeded=true` and `cleanup_errors=[]` in addition to the existing direct-wait and worker-join assertions. Inspect original reports/trace snapshots; successful samples cannot supersede historical failure evidence.

## Review and release boundary

Confirm main and the unrelated experiment stash remain unchanged while developing. Confirm lockfiles, engine payload, M3 snapshots, deadlines, quarantine retries and source/no-clobber contracts are unchanged. Public errors retain schema v1 and selected-engine metadata. Keep issue #27 open. Creation-time Job assignment, cancellable I/O, bounded wait/join, pre-guard cleanup and kernel-level failure injection are not accepted by this PR. No Release or tag is created and every developer candidate remains `public_release_ready=false`.
