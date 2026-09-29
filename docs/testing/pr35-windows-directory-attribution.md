# PR #35 — Windows exact-directory attribution acceptance

This PR is diagnostics-only. Base it on main after #32 and keep PR #34/#33 changes out of the branch. Issue #27 stays open.

## Portable gate

```sh
python3 -B -m unittest discover -s tools -p 'test_windows_directory_users.py' -v
python3 -B -m unittest discover -s tools -p 'test_*.py' -v
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
git diff --check
```

Portable parser/identity tests must reject invalid native lengths/PIDs, PID reuse and unconfirmed identities. Windows-only controls may skip off Windows but must not be counted as acceptance evidence.

## Required Windows calibration

Run:

```powershell
python -B -m unittest discover -s tools -p "test_windows_directory_users.py" -v
```

All three native cases must execute rather than skip:

- known holder is identity-confirmed, remains blocking while the collector runs, disappears after the test releases only its own handle, then rename succeeds;
- delete-sharing holder is observed while rename succeeds, with `rename_blocker_proven=false`;
- sibling-only holder is not attributed to the requested target directory.

Retain the three JSON receipts in `target/windows-directory-calibration/` and the normal Windows resource-calibration artifact. A native unsupported/denied/malformed result fails this calibration; do not silently downgrade it to an empty list.

## Failure-time capture gate

The collector must write these before the existing file/process stages:

```text
02a-directory-users.json
02b-directory-correlation.json
```

For the existing controlled held-directory lifecycle failure, require the known test holder to appear as an identity-confirmed exact-directory user when this native class is supported on the runner. The original OS5 error must stay unchanged and the collector must not release the holder.

For spontaneous real-engine OS5 failures, preserve all directory users with PID + creation FILETIME and correlation to Yu direct-child traces. Do not call any unmatched user "external" solely because it did not match a direct child; descendants/races can remain unrecorded. Do not claim a confirmed directory user caused rename failure without independent share-mode/causal evidence.

## Package workflow

Changing the failure-time collector or exact-directory module must trigger the existing Managed ag-psd Package workflow. Do not rerun a failed package job to replace evidence. Keep the two-second quarantine budget, default parallelism and existing first-failure behavior unchanged.

## Merge boundary

A green PR may establish that the directory-user diagnostic is calibrated on the hosted Windows filesystem and safely attached to failure capture. It does not fix OS5, close Issue #27, set `root_cause_fixed=true`, or authorize v0.1 publication.
