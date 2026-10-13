# M4c-7e PSD Research Exit — Acceptance

**Status:** research exit accepted, PSD production writes blocked.

- [x] 13 valid PSD/PSB samples structurally scanned and classified by
      independent psd-tools layer type; all remain mutation denied.
- [x] Three additional Photoshop 2026 read-only source-copy
      inspections; no Photoshop saves or original changes.
- [x] Native Background localization differences remain excluded.
- [x] Existing raw metadata unknowns do not turn into an ignore-list
      or an automatically expanded writer admission set.
- [x] Source SHA-256 bindings retained and v0.1 `main` frozen.
- [x] M4c-7d native render and source-vs-save preview differences
      explained at the level supported by two independent control runs.
- [x] M4c-7d Slices Resource 1050 name-field delta classified for the
      exact saved group fixture; EXIF/XMP/`shmd` semantics remain
      unresolved and blocked.
- [x] Machine-readable M4c-7e release decision **disallows all PSD writes**.

## Reproduce read-only evidence

```bash
NODE22=target/m4c7d-node22/node_modules/node/bin/node
PYTHON=target/m4c7d-py312/bin/python

# Explicit collection from known frozen corpus, no Photoshop save:
M4C7D_PYTHON="$PYTHON" "$NODE22" \
  crates/yu-psd-spike/adapters/typescript/m4c7e_corpus_admission.cjs --write

# Optional native read-only copy inspection, only if Photoshop is
# responsive with zero open user documents:
M4C6_PYTHON="$PYTHON" "$NODE22" \
  crates/yu-psd-spike/adapters/typescript/m4c7e_native_readonly_sampling.cjs --write

# Offline-only snapshot checks:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c7e_corpus_admission.cjs --check
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c7e_native_readonly_sampling.cjs --check
```

No new Photoshop mutating test cases are admitted as a side effect of
the read-only profiling; `PSD_PRODUCTION_MUTATION_BLOCKED` remains
the final stage result.

## Local regression

- [x] Combined M4c-3 through M4c-7e pinned Node.js 22 PSD suite:
      **56 passed, 0 failed**.
- [x] Rust workspace serial `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** (unchanged Rust workspace baseline).
- [x] Rust Clippy `--workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] Rustfmt, JavaScript syntax and Git whitespace checks: passed.
- [x] All 3 native Photoshop read-only experiment copies and original
      source SHA-256 values unchanged.
