# M4c-9 — Multilingual Legacy Encoding, lyid Identity and Metadata

**Start:** `develop@24440751bf059d8696ce639e6d603ddc611d028b`

**Status:** bounded research acceptance; public PSD mutation blocked.

## Completed

- [x] Reuse previous M4c-8 *Photoshop-generated* sample PSD files
      only after checking **exact prior receipt SHA-256**.
- [x] Observe actual Chinese and Cyrillic legacy Pascal raw bytes,
      which decode as the original names under GB18030 in this locale.
- [x] Preserve legacy raw bytes in evidence; do not promote GB18030
      as a universal PSD codec.
- [x] Detect two native duplicate-name pixels with distinct
      `lyid=6` and `lyid=2`.
- [x] Bind fixed-width ASCII Unicode/Pascal patch by
      `lyid=6` and verify all other source bytes are unchanged.
- [x] Photoshop DOM layer IDs agree with `lyid`, both names
      survive native save/close/reopen on a scratch copy.
- [x] Original and experimental source bytes remain unchanged,
      no user PSD destination is overwritten.
- [x] Extract bounded `shmd/cust` `layerTime` descriptor shapes,
      XMP changed field markers and EXIF byte-delta counts.
- [x] No public PSD mutation, UI/engine registration, PR or
      production writer release.

## Reproduction

```bash
NODE22=target/m4c9-node22/node_modules/node/bin/node

# Explicitly run only against the prior M4c-8 Photoshop scratch run,
# under the same host's ignored target/m4c8-multilingual-native/run-*:
M4C6_PYTHON=target/m4c9-py312/bin/python "$NODE22" \
  crates/yu-psd-spike/adapters/typescript/m4c9_legacy_lyid_research.cjs \
  --write /absolute/path/to/prior/m4c8-multilingual-native/run-...

# These two exact M4c-7d Photoshop saved run directories are
# inspected read-only and source hashes must match committed receipts.
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c9_metadata_semantics.cjs \
  --write /absolute/path/to/m4c7-psd-photoshop-save/run1 \
          /absolute/path/to/m4c7-psd-photoshop-save/run2

# Offline snapshot checks never open Photoshop or modify a PSD:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c9_legacy_lyid_research.cjs --check
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c9_metadata_semantics.cjs --check
"$NODE22" --test \
  crates/yu-psd-spike/adapters/typescript/m4c9_encoding_identity.test.cjs
```

## Exit gate

- [x] Duplicate `lyid` identity research and non-ASCII legacy
      sample decoding evidenced in versioned machine receipts.
- [x] Metadata classifier refuses unrecognized custom descriptors
      and does not silently ignore changed EXIF/XMP/`shmd` payloads.
- [ ] All user documents, Photoshop locales and arbitrary PSDs:
      **not certified**.
- [ ] Production output transaction, writer admission, broad metadata
      preservation: **not authorized**.

**PSD_PRODUCTION_MUTATION_BLOCKED**.

## Completed local validation

- [x] Combined M4c-3 through M4c-9 Node.js 22.23.3 PSD regression:
      **66 tests passed, 0 failed**.
- [x] Rust workspace `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** (macOS).
- [x] Rust Clippy `--workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] Rustfmt, JavaScript syntax and Git whitespace checks: passed.
- [x] Offline machine evidence checks passed without launching
      Photoshop, re-running a native save, or overwriting sources.
- [x] Photoshop was responsive with **zero** open documents after tests.
