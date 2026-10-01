# PR #37 — Windows in-flight regular-file sampling acceptance

This is a diagnostics-only follow-up from `main@5a13e7e249e5b88b6a1313bcff7ea1acaabd8012`. PR #34 startup and PR #33 distribution remain separate.

## Local gate

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 -B -m unittest discover -s tools -p 'test_windows_remove_file_sampler.py' -v
python3 -B -m unittest discover -s tools -p 'test_*.py' -v
git diff --check
```

The new native sampler calibration skips outside Windows and is not accepted on the basis of those skips.

## Native Windows calibration

Run:

```powershell
python -B -m unittest discover -s tools -p "test_windows_remove_file_sampler.py" -v
```

Required controls:

1. **Transient nested-file holder** — hold `runtime/node.exe` without FILE_SHARE_DELETE, release at about 500 ms, and verify rename retry first fails then succeeds while a PID + creation-time singleton witness for `runtime/node.exe` is retained.
2. **Persistent metadata holder** — hold `.yu-install.json` beyond two seconds and verify rename remains failed while a singleton witness for that exact file is retained.
3. **Sibling-file isolation** — hold only a sibling version's file and verify the target rename succeeds immediately without attributing the test PID to the target.

The sampler must never release the test holder. Calibration receipts are retained under `target/windows-remove-file-calibration/`.

## Rust integration control

The existing Windows `held_nested_file_preserves_installation_and_other_active_version_until_explicit_release` test already owns a nested file handle and expects the first remove to fail closed.

When `YU_TEST_REMOVE_FILE_SAMPLING_DIR` is enabled, that same test must additionally observe exactly one matching file-sampling report containing the test process as a singleton witness for `runtime/fixture.exe`. The original CLI error, file bytes, installed versions and held handle must remain unchanged.

## Real lifecycle evidence

Run Windows Lifecycle Diagnostics and the Windows Managed Package gates with both PR #36 exact-directory sampling and PR #37 regular-file sampling enabled.

On a spontaneous OS5:

- do not retry the failed case;
- preserve the original error and quarantine attempts/elapsed;
- compare exact-directory sampling, regular-file identities/witnesses, post-failure resource users and Yu-owned process traces by PID + creation FILETIME;
- retain unresolved identities explicitly when the relationship disappears before a singleton can be proven.

A singleton witness is still **not** a rename-blocker proof. No release gate changes until Issue #27 is separately resolved.

## Merge boundary

This PR may be accepted as diagnostic infrastructure if calibration and ordinary CI/package gates are sound even when the lifecycle workflow intentionally retains a spontaneous Issue #27 failure. It does not widen retries, serialize tests, terminate external processes, close Issue #27 or authorize v0.1.
