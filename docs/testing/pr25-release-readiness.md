# PR #25 — v0.1 release preparation acceptance

Base: `46c3753817ffa1a3f7beb804f3da178c9eab19cf`. Scope and unaccepted release gates: [v0.1 readiness](../releasing/v0.1-readiness.md).

## Ordinary tests

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
python3 -B -m unittest discover -s tools -p 'test_*.py' -v
```

Offline installer tests cover successful verified installation without activation, no network route, source preservation, version conflict, checksum mismatch, wrong platform, bad archive, bounded local bytes, non-files and Unix symlinks. Existing archive safety and M3 freeze guards must remain green. Quarantine retry/error tests and the Windows real directory-handle regression remain required.

## Real package and release executable

Run Managed ag-psd Package. All three targets must pass the original engine smoke, actual read-only/pixel tests and ten-cycle lifecycle test; then build the developer candidate and rerun those tests against its extracted release executable. `YU_TEST_CLI` selects that binary explicitly. No failed-cycle retry or serial-test workaround is allowed.

```bash
# After building the local engine and CLI candidate, set absolute paths:
export YU_TEST_MANIFEST="$PWD/target/ag-psd-managed/macos-aarch64/manifest-macos-aarch64.json"
export YU_TEST_PACKAGE="$PWD/target/ag-psd-managed/macos-aarch64/yu-engine-ag-psd-31.0.2-node22.23.3.yu2-macos-aarch64.zip"
export YU_TEST_CLI="$PWD/target/cli-candidate/macos-aarch64/smoke/bin/yu"
cargo test --locked -p yu-cli --test psd_managed -- --ignored --nocapture
```

Record source commit, run ID/attempt, target, completed cycles, quarantine retries, three test results, package digest and candidate checksum. On a failure, retain the original diagnostic and mark that target unaccepted. Reruns do not establish a root-cause fix. Do not claim local real-package execution from hosted evidence.

## Archive / release boundary

Check that candidate ZIP extraction rejects unexpected names, symlinks/special files, duplicate names and binary tampering. Verify the inner candidate checksum, not the Actions wrapper checksum. Run `smoke/bin/yu --version` and confirm build-info source/target. The archive has no engine bundled and no accepted signing/license status. Existing candidate destinations must not be overwritten.

The candidate builder refuses dirty/uncommitted input and missing/untracked locks. Both clean pre-build and post-build source checks must pass. Generated directories belong below `target/`; no user engine storage is touched by tests.

Expected result: engineering preparation validated on the exact commit; **public_release_ready remains false**. Do not create a release tag or publish as part of this checklist.
