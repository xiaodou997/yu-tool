# M4a raster operations acceptance

Feature branch: `feature/m4a-raster-operations`. Base:
`main@5526dc4465c4e79ad7f2757d06b3b1693642b8af`.
M4a must not change the v0.1 technical RC or release-readiness claims.

## Scope

- `yu image crop INPUT --x X --y Y --width W --height H -o OUTPUT [--json]`
- `yu image rotate INPUT --degrees 90|180|270 -o OUTPUT [--json]`
- `yu image convert INPUT -o OUTPUT [--json]`
- PNG/JPEG/WebP encoding through the existing `raster-rs` engine.
- Capability registry/engine inventory and schema-v1 result/error envelopes.
- Non-destructive output publication (completed temp file, atomic hard-link
  insertion, temp cleanup). Unsupported hard-link filesystems fail closed.

## Acceptance checks

- [x] `cargo fmt --all -- --check` (local macOS)
- [x] `cargo check --workspace --all-targets --locked` (local macOS)
- [x] `cargo test --workspace --locked` (local macOS)
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings` (local macOS)
- [ ] CI on Linux, macOS and Windows.
- [ ] crop pixels/dimensions accurate; zero, out-of-bounds and u32 overflow rejected.
- [ ] rotate direction/geometry correct; non-right-angle rejected.
- [ ] PNG → JPEG and WebP round-trip decode; source unchanged.
- [ ] existing output never overwritten and unsupported format rejected.
- [ ] v0.1 main SHA unchanged; no tag, Release, notarization, or public upload.

## Deferred

Arbitrary-angle rotation, batch operations, metadata preservation, color
profiles, output replacement/dry-run, PSD mutation and GUI.
