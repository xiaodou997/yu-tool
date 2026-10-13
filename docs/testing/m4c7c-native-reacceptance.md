# M4c-7c Native Photoshop Save/Reopen — 2026-10-13

**Base:** `develop@e0c8af6481ae4c3b86b80485ac14f510bedcfea0`

**Main:** v0.1 release candidate remains frozen at
`5526dc4465c4e79ad7f2757d06b3b1693642b8af`.

## Confirmed native sequence

- [x] Photoshop 2026 restarted by user and responds to Apple Events.
- [x] No user documents open before test (`open_documents=0`).
- [x] Only `nested-group` admitted via frozen M4c-7a research policy.
- [x] Save unmodified original copy and patched copy to two distinct
      new paths in an ignored `target/` run directory.
- [x] Close each experiment document without saving its input.
- [x] Photoshop reopen confirms `Group 1` vs `Group 2`.
- [x] Independent parser confirms expected logical tree in each output.
- [x] Independent saved-control and saved-candidate RGBA previews match.
- [x] Raw paired channels and compressed saved composite byte hashes match.
- [x] Original fixture SHA-256 and disposable input SHA-256 unchanged.
- [x] Historical native-unresponsive receipt preserved in v2.
- [x] New real Photoshop saved-pair receipt recorded in v3.
- [x] Programmed `--check` is *offline only*, never runs Photoshop saves.

## Explicit unresolved fidelity issues

- [ ] The independently decoded source vs Photoshop-saved preview differs
      even on the unmodified original control; further native pixel and
      cached-composite analysis is required.
- [ ] The two saved outputs differ in serialized resource `1060`,
      `1050`, `1058` and multiple `shmd` payloads.
- [ ] Two saved outputs have different lengths (33,256 vs 33,240 bytes).
- [ ] Strict preservation of all non-name bytes after Photoshop saveAs
      is **not** established. Saved-pair strict gate remains FAILED.
- [ ] No Photoshop mask/effect/ICC/Smart Object multi-fixture fidelity
      study, cross-platform replay, version-bound published output or
      public PSD mutation permission.

## Reproduce

```bash
npm ci --ignore-scripts --no-audit --no-fund --prefix packaging/ag-psd-engine
NODE22=/path/to/node-v22.23.3
# Ensure Photoshop is open, responsive, and has zero documents before
# explicitly invoking the native research experiment:
M4C6_PYTHON=/path/to/python3.12-with-psd-tools-1.20.0 \
  "$NODE22" crates/yu-psd-spike/adapters/typescript/m4c7_photoshop_save_probe.cjs --write

# Safe offline receipt integrity check: no new PSD output is created.
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c7_photoshop_save_probe.cjs --check

"$NODE22" --test \
  crates/yu-psd-spike/adapters/typescript/m4c3_raw_block_inventory.test.cjs \
  crates/yu-psd-spike/adapters/typescript/m4c7_scope_freeze.test.cjs \
  crates/yu-psd-spike/adapters/typescript/m4c7c_native_readiness.test.cjs \
  crates/yu-psd-spike/adapters/typescript/m4c7c_saved_pair_gate.test.cjs \
  crates/yu-psd-spike/adapters/typescript/m4c7c_native_reacceptance.test.cjs
```

- [x] PSD scanner + native research suites: **30 passed, 0 failed**.
- [x] Final Rust workspace
      `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** on local macOS.
- [x] Rustfmt `cargo fmt --all -- --check`: passed.
- [x] Clippy `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] JavaScript syntax and Git whitespace checks: passed.
- [x] Photoshop native readiness after experiment:
      `ready_empty_workspace`, **zero open documents**.

**Release decision:** `PSD_PRODUCTION_WRITE=BLOCKED`.

See [ADR 0021](../decisions/0021-photoshop-native-save-reopen-reacceptance.md)
and [v3 receipt](../data/m4c7c-photoshop-native-reacceptance-v3.json).
