# PR #32 — Combined Windows startup / transport / cleanup acceptance

Base: #31 squash `4a939e4b7c23622ce445aeb3a680c2678b0154ea`. Review this adaptation independently; do not merge old #29 or overwrite its failed evidence. No publishing.

## Source and local gate

Record exact main, feature and hosted test-merge SHAs; match candidate build-info and binary hashes to the checkout actually used. Confirm the saved experiment stash and original failures remain unchanged. Cargo.lock, the pinned toolchain, engine payload, protocol/corpus and quarantine code must match the base.

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 -B -m unittest discover -s tools -p 'test_*.py' -v
```

Portable Windows serialization tests execute on non-Windows too; native startup tests require actual Windows. Skips/helpers are not counted as substantive native scenarios.

## Native joint gate

```sh
cargo test --locked -p yu-runtime-psd --lib process::windows_spawn::tests -- --nocapture
cargo test --locked -p yu-runtime-psd --lib process::bounded_tests -- --nocapture
```

Require all eight substantive startup scenarios, the bookkeeping guard and eight retained-peer/bounded scenarios to execute. Check early-descendant membership before input, the unchanged one-second inherited-output timeout and four-second outer bound, actual retained-handle exit before the 20-second fixture backstop, two independent overlapping invocations, 64 KiB input, query-only creation denial, foreign-Job retention refusal and real accounting-rights denial after response. The original first-run failure must remain recorded; do not add waiting to its final test assertion. Production captures/validates members before termination and polls their stable handles inside the same existing deadline. No infinite waiter or test retry should hide failure. Existing PNG no-publication and racing-writer guards remain.

## Real-package gate

Run the three-platform Managed Package workflow, including read-only/pixel/lifecycle checks against the unpacked release-mode candidate, and the existing Windows three-pass probe. Keep the operation/quarantine deadlines, parallelism and first-failure stop unchanged. Retain any failure and the completed-cycle count; do not replace a failed run with an unexplained rerun.

Inspect original Windows artifacts: actual source/candidate identities and input hashes, all cleanup stages, zero workers, closed endpoints, direct exit, Job-empty confirmation and retained_members_confirmed. Snapshot truncation/identity failure or late membership cannot count as success. Distinguish expected OS32 controls from spontaneous OS5. A positive file-user observation is still not causal blocker proof. Do not reuse prior #29/#31 workflow greens as joint evidence.

## Windows uninstall gate follow-up

Package26 on `0ff5e9e` failed its real read-only test's final quarantine (OS5, 67 attempts, 2007ms). This remains a failed positive gate, not an expected negative test. Its failing invocation had no enabled occupancy/owned-process capture; passing Diagnostics11 cannot establish its cause.

The package workflow now enables the EXISTING test-only collector and trace separately for debug and extracted-release tests. It retains original Cargo output with pipeline failure propagation, exact checkout/workflow identity, post-attempt input hashes and same-invocation snapshots even on failure. No test is retried, no collector API expands, no operation/quarantine budget changes, and a failed phase still skips candidate promotion and manifest assembly. Trace-enabled success can have timing effects and is not proof that an unobserved failure was fixed.

Run the additional actual-Windows control:

```sh
cargo test --locked -p yu-cli --test psd removal_contract -- --nocapture
```

The fixture holds a nested regular file (not the version directory and not an executed engine), then removes that inactive version while another version remains active. Require EXECUTION_FAILED, original metadata/executable/source hashes and both inventory entries unchanged, the other active version unchanged, and an intact holder after capture. With collection enabled, require exactly one matching original-error context and a completed capture. Only after the fixture explicitly closes its own handle does a NEW CLI invocation remove the target version. This is a changed-precondition control, not a hidden retry or evidence about the historical holder. The control's allowed native denial family is5/32/33; retain the actual code rather than forcing a causal OS5 interpretation.

Report the native fail-closed contract, real positive package acceptance, and historical root-cause status separately. The control does not waive the positive gate. Do not move a failed real uninstall into a passing bucket or accept only the independent trace-enabled probe. Record the exact shell/logging change (Windows package Cargo output now uses Bash with pipefail/tee) as part of the new test environment.

## Delivery

Document review findings and exact-head gates. Leave the new PR open until reviewed; leave #29's old head/history unchanged. Update issue #27 without marking historical OS5/30-second timeout fixed. No release, signatures, minimum-system certification or candidate promotion.
