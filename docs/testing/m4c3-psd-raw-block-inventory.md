# M4c-3 Read-only PSD Raw Block Inventory Acceptance

Base: `develop@00d0c2fb2b67f3fc5561374498d2fb3365358f92`

Research branch: `research/m4c3-psd-raw-block-inventory`

## Frozen scope

- [x] Header/section bounds, PSD/PSB u32/u64 lengths, overflow guards.
- [x] Resource signature/ID/Pascal name/data length and even padding.
- [x] Per-layer channel lengths, masks, blending ranges, Pascal names,
      layer-tagged blocks and global tagged blocks.
- [x] Block offset, length, recognized/unknown and SHA-256 inventory.
- [x] Unknown IDs and unknown keys survive in structured evidence;
      recognized-but-opaque payloads are **not** considered safe.
- [x] Embedded high-bit PSB layer records explicitly flagged as unparsed.
- [x] Noncanonical padding reported as risk instead of inferred safety.
- [x] Decompressed merged-image corruption is *not* declared detectable.
- [x] Every accepted input returns `mutation_authorized=false` and
      `safe_to_rewrite=false`.
- [x] No production PSD mutation engine/CLI/protocol change; v0.1 main intact.

## Local macOS verification

```bash
node --test crates/yu-psd-spike/adapters/typescript/m4c3_raw_block_inventory.test.cjs
node crates/yu-psd-spike/adapters/typescript/m4c3_raw_inventory_corpus.cjs --check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
git diff --check
```

- [x] Scanner tests (including signature/length/unknown metadata fuzz):
      10 passed, 0 failed on Node 26 local macOS.
- [x] Deterministic machine corpus report: 13 valid inputs inventoried,
      1 known malformed rejected, 11/13 valid inputs have unknown block IDs
      or keys.
- [x] Rust workspace `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** (local macOS).
- [x] Rustfmt `cargo fmt --all -- --check`: passed.
- [x] Clippy `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] Git diff whitespace check and Node.js scanner/test source syntax: passed.
- [x] Deterministic snapshot `--check`: passed.
- [ ] Independent Windows/Linux runs.
- [ ] Real Photoshop fidelity and embedded/opaque block preservation.

The last two are deliberate *future production gates*, not requirements for
finishing the read-only research scanner. No GitHub Actions quota is used
in this slice.

## Reproduction

Scan a named local PSD without writing it:

```bash
node crates/yu-psd-spike/adapters/typescript/m4c3_raw_block_inventory.cjs \
  fixtures/psd/upstream/psd-tools/2layers.psd
```

The JSON result contains `inventory.header`, section boundaries,
`image_resources`, `layer_records`, `tagged_blocks`, `unknown_blocks`,
`risks`, and `mutation_authorized=false`. Invalid structure produces a
structured error with a byte offset and a nonzero exit code.
