# M4b-1 image dry-run acceptance

Base: `develop@990a237be4a1505c7d49dd87c460560c31a63768`.
Branch: `feature/m4b1-image-dry-run`. Frozen `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af`
is unaffected.

## Contract

- Add `--dry-run` to `image.resize`, `image.crop`, `image.rotate`, `image.convert`.
- Return existing typed fields plus additive `result.dry_run` boolean.
- Decode input, validate geometry and format, check destination absence
  (including dangling symlinks) and existing parent directory.
- No output encoders, temporary output files, filesystem mutations or
  content replacement during dry-run.
- Normal execution retains no-clobber, atomic hard-link publication; a
  destination racing into existence remains protected by the link operation.
- Preflight is advisory and does not guarantee subsequent write success.

## Local checks

- [x] `cargo fmt --all -- --check` (local macOS)
- [x] `cargo check --workspace --all-targets --locked` (local macOS)
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings` (local macOS)
- [x] `cargo test --workspace --locked` (local macOS, 160 passed / 0 failed)
- [x] `git diff --check` (local macOS)
- [x] Four dry-run operations: dimensions, formats and no output files.
- [x] Existing destination, missing parent and invalid crop rejected.
- [x] Source file remains unchanged.

Cross-platform GitHub Actions is intentionally not a required gate for
this development slice while Actions quota is unavailable. Do not claim
three-platform coverage without corresponding evidence.

## Deferred

Safe replacement/overwrite with version binding, `--dry-run` for PSD,
batch planning, transaction tickets and public releases.
