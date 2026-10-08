# M4b-2: version-bound safe output replacement

Base: `develop@fbe1bf9719a5b42c1035c29edb1b0e7ae958a069`.
Implementation branch: `feature/m4b2-version-bound-replace`.
Frozen technical v0.1: `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af` (unchanged).

## Product contract

- No flags: create a new raster output, no clobber.
- `--replace --expected-output-sha256 <64-hex>`: existing *regular*
  destination only, exact content hash guard; invalid/missing/mismatched
  targets fail closed. Either option alone is rejected by CLI parsing.
- Opt-in same-input/output mutation requires that exact guard.
- For real replacement: exclusive create-new YuTool sidecar lock, encode into
  unique create-new file in the same directory, flush/sync stage, verify
  destination again, then rename. Remove stage/lock on normal completion
  and failure. Windows rename may fail under open file sharing.
- For dry-run: validate exact destination digest/geometry and return plan;
  **no** sidecar, stage, encoding, publish or other filesystem changes.
- Schema-v1 operation results gain `replaced` and `would_replace` booleans.

## Explicit limitation

The second SHA-256 check is immediately before OS rename but cannot
atomically compare-and-swap a destination concurrently mutated by a
non-cooperating external program. Do not claim complete CAS isolation.
No durability guarantee across sudden process kill/power loss; an orphaned
sidecar file can require manual inspection/removal. Replacements do not
preserve previous inode/hardlinks, modes or extended metadata.

## Acceptance

- [x] `cargo fmt --all -- --check` (local macOS)
- [x] `cargo check --locked --workspace --all-targets` (local macOS)
- [x] `cargo clippy --locked --workspace --all-targets -- -D warnings` (local macOS)
- [x] `cargo test --locked --workspace` (local macOS; 170 passed, 0 failed)
- [x] `git diff --check` (local macOS)
- [x] All four mutations replace only when expected digest matches.
- [x] Default/no flags still refuses existing output.
- [x] Dry-run validates and reports `would_replace`, with zero writes.
- [x] Mismatched/malformed SHA, missing destination, directory and symlink rejected.
- [x] Failed after-stage recheck preserves newer destination bytes.
- [x] Cleanup and exclusive-cooperating-lock behavior tested.
- [ ] Test on Windows independently when a local device is available.

With GitHub Actions quota exhausted, no new PR/Actions gate will be
triggered. Local macOS evidence is not mislabeled three-platform acceptance.
