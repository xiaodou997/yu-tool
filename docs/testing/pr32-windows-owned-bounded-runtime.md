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

Require all seven startup scenarios and eight retained-peer/bounded scenarios to execute. Check early-descendant membership before input, the one-second inherited-output timeout, actual retained-handle exit before the 20-second fixture backstop, two independent overlapping invocations, 64 KiB input, query-only creation denial and real accounting-rights denial after response. No infinite waiter or test retry should hide failure. Existing PNG no-publication and racing-writer guards remain.

## Real-package gate

Run the three-platform Managed Package workflow, including read-only/pixel/lifecycle checks against the unpacked release-mode candidate, and the existing Windows three-pass probe. Keep the operation/quarantine deadlines, parallelism and first-failure stop unchanged. Retain any failure and the completed-cycle count; do not replace a failed run with an unexplained rerun.

Inspect original Windows artifacts: actual source/candidate identities and input hashes, all cleanup stages, zero workers, closed endpoints, direct exit and Job-empty confirmation. Distinguish expected OS32 controls from spontaneous OS5. A positive file-user observation is still not causal blocker proof. Do not reuse prior #29/#31 workflow greens as joint evidence.

## Delivery

Document review findings and exact-head gates. Leave the new PR open until reviewed; leave #29's old head/history unchanged. Update issue #27 without marking historical OS5/30-second timeout fixed. No release, signatures, minimum-system certification or candidate promotion.
