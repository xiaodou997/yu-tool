# M4c-7d — Native Photoshop Rendering and Metadata Differential Acceptance

Date: **2026-10-13**

Original `develop` experiment baseline:
`80af5544293a289264759cfa89ea39d28f43cf51`.

## Research conclusions

- [x] Native Photoshop 2026 saveAs control vs patched group experiments:
      two independent runs, each with an unchanged original control.
- [x] Four native Photoshop PNG exports per run:
      original/renamed input, saved unchanged control, saved renamed
      candidate. Native PNG pixels agree **0 changed** across each run.
- [x] psd-tools original PSD composite differs from Photoshop native
      PNG in exactly **160/20,000 pixels**, while both saved PSDs'
      independent composites agree with native PNG exactly.
- [x] Pixel-difference bounding box `[26,25,65,97)`,
      maximum channel delta 11, alpha unchanged.
- [x] Byte-level reason for ID 1050: Slices name
      `nested-group-control` vs `nested-group`, 8 UTF-16 code
      units = 16 bytes, with identical surrounding bytes.
- [x] Identify ID 1060 as XMP with changed document/history
      markers, 1058 as EXIF, and `shmd` as four changed
      52-byte `cust` metadata items. Their full semantics **not**
      independently certified.
- [x] All analysis tools and saved report snapshots are research-only.
- [x] Original v0.1 main release baseline untouched.

## Reproduction in the isolated worktree

```bash
# Existing scratch runs (do not perform additional native PSD saves).
RUN1=target/m4c7-psd-photoshop-save/run-EX35Cv
RUN2=target/m4c7-psd-photoshop-save/run-arUw9W
NODE22=target/m4c7d-node22/node_modules/node/bin/node

# Read-only normalized composite/RGB and resource report.
M4C7D_PYTHON=target/m4c7d-py312/bin/python "$NODE22" \
  crates/yu-psd-spike/adapters/typescript/m4c7d_photoshop_delta_audit.cjs \
  --write "$RUN1" "$RUN2"

"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c7d_metadata_forensics.cjs \
  --write "$RUN1" "$RUN2"

# Offline-only checks: no Photoshop invocation.
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c7d_photoshop_delta_audit.cjs --check
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c7d_metadata_forensics.cjs --check
"$NODE22" --test \
  crates/yu-psd-spike/adapters/typescript/m4c7d_photoshop_delta_audit.test.cjs

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
```

- [x] Full M4c-3 through M4c-7d Node 22.23.3 PSD regression:
      **52 passed, 0 failed**.
- [x] Rust workspace `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** (local macOS).
- [x] Rustfmt `cargo fmt --all -- --check`: passed.
- [x] Clippy `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] JavaScript syntax and Git whitespace checks: passed.

## Unresolved product gate

- [ ] Independent meaning/retention of EXIF, XMP, `shmd/cust` metadata.
- [ ] New source fixtures for ordinary pixel and non-ASCII Unicode/legacy
      names, duplicate logical names, masks/effects and RGB/PSB
      variants with independently checked native Photoshop semantics.
- [ ] Safe general output publication and production engine contract.
- [ ] M4c-7e decision gate for public PSD mutation: **BLOCKED**.

See [ADR 0022](../decisions/0022-photoshop-m4c7d-metadata-fidelity-boundary.md).
