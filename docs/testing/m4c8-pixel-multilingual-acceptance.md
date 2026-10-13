# M4c-8 — Ordinary Pixel and Multilingual PSD Rename Acceptance

**Base:** `develop@30db4036368e3961213f4e69555dda9d51f00d35`

**Status:** Validated research; no PSD public writer.

## Implemented

- [x] Exact SHA-pinned 8-bit RGB ordinary pixel source and selected
      raw layer index 0; prevent anonymous or user-supplied PSD input.
- [x] Fixed-width `Layer 1 → Layer A` changes only two bytes across
      the Unicode `luni` and matching ASCII Pascal names.
- [x] File length, raw channels, Image Resources, merged composite
      and original source SHA-256 unchanged before native save.
- [x] Photoshop 2026 native saveAs, close/reopen for unchanged
      original control and edited candidate, with zero user docs open.
- [x] After Photoshop save, correct name in both Photoshop and
      psd-tools; saved control/candidate independent composites match.
- [x] Additional Photoshop-generated scratch-name fixtures:
      Chinese `测试图层`, Cyrillic `Слой`, duplicate `Layer 0`.
- [x] Multi-language Unicode-only byte patches demonstrate read
      recognition but leave the legacy Pascal bytes unchanged;
      **not safe to publish**.
- [x] Every original and scratch input verified unchanged after
      the Photoshop experiment. No user PSD inputs modified.
- [x] Machine-readable receipts committed; binary scratch PSDs
      excluded from Git.
- [x] Research-only deny policy keeps all production mutation blocked.

## Run / inspect

```bash
NODE22=target/m4c8-node22/node_modules/node/bin/node
PYTHON=target/m4c8-py312/bin/python

# Explicit Photoshop scratch experiments; requires responsive/empty
# Photoshop 2026. Never run as unattended CI.
M4C6_PYTHON="$PYTHON" "$NODE22" \
  crates/yu-psd-spike/adapters/typescript/m4c8_pixel_rename_research.cjs --write
M4C6_PYTHON="$PYTHON" "$NODE22" \
  crates/yu-psd-spike/adapters/typescript/m4c8_multilingual_native_probe.cjs --write
M4C6_PYTHON="$PYTHON" "$NODE22" \
  crates/yu-psd-spike/adapters/typescript/m4c8_luni_legacy_divergence.cjs --write

# Snapshot validation is offline and does NOT execute native saves:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c8_pixel_rename_research.cjs --check
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c8_multilingual_native_probe.cjs --check
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c8_luni_legacy_divergence.cjs --check
"$NODE22" --test crates/yu-psd-spike/adapters/typescript/m4c8_pixel_multilingual.test.cjs
```

## Completed local regression

- [x] Entire M4c-3 through M4c-8 PSD/Node.js 22.23.3 suite:
      **61 passed, 0 failed**.
- [x] Rust `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** on local macOS.
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- [x] `cargo fmt --all -- --check`: passed.
- [x] JavaScript syntax, offline machine receipt checks and Git
      whitespace checks: passed.
- [x] Native Photoshop document count following experiment: **0**.

## Outstanding production gates

- [ ] Multilingual Photoshop legacy Pascal encoding and round-trip mapping.
- [ ] Duplicate-layer stable identity binding, not name-based matching.
- [ ] EXIF/XMP/Slices/`shmd` metadata semantic preservation on
      arbitrary real PSD files.
- [ ] Native Photoshop visual/metadata fidelity on other PSD
      feature variants, Windows/Linux replay and safe output publishing.
- [ ] Independent explicit release approval for public `yu psd rename`.

**PSD_PRODUCTION_MUTATION_BLOCKED**.
