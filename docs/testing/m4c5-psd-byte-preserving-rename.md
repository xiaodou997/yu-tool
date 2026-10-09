# M4c-5 PSD Fixed-width Byte Patch: Local Research Acceptance

Baseline: `develop@3c8769c93389f9d24847df2c3e9748852d57eb7d`

Feature: `research/m4c5-byte-preserving-layer-rename`

Scope: limited research only. No public PSD write command or change to
the frozen v0.1 `main`.

## Research implementation gates

- [x] Exact M4c-2 SHA-256 fixture identity and pinned ag-psd/Node runtime.
- [x] M4c-2 read-only preflight remains `write_authorized=false`.
- [x] Canonical layer ID checked independently of the raw PSD record index.
- [x] Selected `luni` offset and UTF-16BE name verified before mutation.
- [x] All proposals preserve the original UTF-16 code-unit count and file size.
- [x] Only exact allowed byte ranges differ; all other bytes identical.
- [x] Legacy Pascal name optionally synchronized when ASCII and same length;
      divergence is surfaced for all other cases.
- [x] Source unchanged, disposable candidate created with `wx` under ignored
      `target/` and hashed after reading back.
- [x] Independent raw scan and ag-psd read check layer identity, tree,
      resource and compressed image/channel equality.
- [x] Negative tests include wrong source version, wrong canonical/raw ID,
      malformed `luni` length, different name length, invalid Unicode,
      conflicting output and non-rename operation.
- [x] No production capability registration, release tags, PR/CI execution,
      engine manifest or PSD writer added to `yu`.

## Local commands and evidence

```bash
npm ci --ignore-scripts --no-audit --no-fund --prefix packaging/ag-psd-engine
NODE22=target/m4c5-node22/node_modules/node/bin/node
"$NODE22" --test crates/yu-psd-spike/adapters/typescript/m4c5_byte_patch_spike.test.cjs
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c5_byte_patch_spike.cjs --check

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
git diff --check
```

- [x] Research fixture runs: 4/4 passed, original files untouched,
      production write authorizations 0.
- [x] Pinned Node22 byte-level safety tests: 5 passed, 0 failed.
- [x] Rust workspace `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** (local macOS).
- [x] Rustfmt `cargo fmt --all -- --check`: passed.
- [x] Clippy `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] Node.js JavaScript source and test syntax: passed.
- [x] Committed evidence reproduced by `--check`.
- [x] Git staged whitespace check (verified before commit).

## Explicitly NOT accepted for product release

- [ ] Cross-editor Photoshop name consistency and alternate legacy fallback.
- [ ] New names with varying UTF-16 code-unit lengths.
- [ ] Arbitrary customer PSD/PSB input or unknown Photoshop metadata.
- [ ] Windows/Linux filesystem publication/lifecycle verification.
- [ ] PSD output-only publication policy with durable versioned audit receipt.

These limitations continue to block production PSD mutation despite
successful controlled byte-level research.
