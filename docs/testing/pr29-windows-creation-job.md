# PR #29 — Creation-time Windows Job acceptance

Use the final committed feature head. Record PR test-merge checkout separately. This checklist is not a success receipt.

## Ordinary checks

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
python -B -m unittest discover -s tools -p 'test_*.py' -v
```

On Windows, explicitly run `cargo test --locked -p yu-runtime-psd --lib process::windows_spawn::tests -- --nocapture`. Require all four actual startup controls and both helper tests to execute, not be skipped. The new/legacy controls must observe membership/non-membership in the exact test Job before request input. Query-only Job access must refuse startup without creating fixture outputs. The argv/cwd/whitelist control must preserve literal values and exclude the unrelated handle by file identity. Existing cleanup failure and PNG publication guards must still execute.

## Actual packaged CLI

Run Managed ag-psd Package on Linux x64, macOS ARM64 and Windows x64, including extracted release executables, independent RGBA fingerprints and ten-cycle lifecycle suites. Run independent Windows Diagnostics with its fixed three passes and inspect original reports. Its 15 old native-case executions /30 real lifecycle cycles are separate from the six new startup module tests; do not inflate either count.

Keep the first failure and exact source/input/candidate identities. Do not rerun a failed case until green, increase timeouts/retries, serialize away races or replace missing tests with source-only assertions. Inspect cleanup snapshots and expected held-directory controls separately from spontaneous OS5 events.

## Scope review

Check no production post-spawn Job assignment/fallback remains. Review startup resources, pointer lifetimes, inherited handles, command/environment encoding and failures before/after CreateProcessW. Confirm no I/O cancellation or new cleanup deadline/whole-Job completion behavior is claimed. Frozen fixtures, engine payload, lockfiles, source/output safety, stash and release-blocked state must stay intact. Issue #27 remains open; #29 must not be merged automatically or treated as formal v0.1.
