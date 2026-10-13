# M4c-10 — Dual-Name Sync / Photoshop Structural ID Acceptance

**Base:** `develop@6002b1b2b844dc48ceb11357cbc814d2a041366e`

**Status:** bounded research; `PSD_PRODUCTION_MUTATION_BLOCKED`

## Scope and native evidence

- [x] Both source multilingual PSDs taken from M4c-8
      Photoshop-generated scratch run, bound by the original
      native-saved SHA-256 receipts.
- [x] Source is a normal 8-bit RGB PSD with exactly two raw layers,
      `lyid=6` target and `lyid=2` unrelated layer.
- [x] `测试图层 → 实验图层` and `Слой → Тест`
      have identical UTF-16BE and GB18030 encoded byte widths.
- [x] Both Unicode and legacy Pascal bytes updated in private source
      copies; unrelated raw PSD bytes and original source hashes unchanged.
- [x] Two native Adobe Photoshop 2026 original controls and edited
      candidates saved separately to new isolated PSD outputs,
      closed, reopened, and both name encodings read back.
- [x] No user document opened or overwritten; Photoshop preflight
      required responsiveness and **zero** user documents.
- [x] Both independently decoded saved composites agree between
      control and edit. No generic renderer fidelity certification.
- [x] Two further native experiments: reorder same-name layers,
      move one into a new Photoshop group. The target `lyid=6`
      and other `lyid=2` remain stable, but raw record indexes
      move and new group-related records/IDs appear.
- [x] Saved-pair raw channels and compressed composite equal in both
      multilingual experiments; resource/metadata differences remain.
- [x] All three machine-readable research receipts are committed.
- [x] No PSD write CLI, public mutation policy, Managed engine or
      v0.1 `main` change.

## Research-only invocation

```bash
# Run only on the original research host with the old M4c-8 isolated
# exact-hash Photoshop sample present under ignored target/.
NODE22=target/m4c10-node22/node_modules/node/bin/node
M4C6_PYTHON=target/m4c10-py312/bin/python "$NODE22" \
  crates/yu-psd-spike/adapters/typescript/m4c10_multilingual_dual_sync.cjs --write

"$NODE22" \
  crates/yu-psd-spike/adapters/typescript/m4c10_lyid_structural_stability.cjs --write
"$NODE22" \
  crates/yu-psd-spike/adapters/typescript/m4c10_metadata_paired_audit.cjs --write

# OFFLINE, no Photoshop edits or PSD saves:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c10_multilingual_dual_sync.cjs --check
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c10_lyid_structural_stability.cjs --check
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c10_metadata_paired_audit.cjs --check
```

## Unresolved gates

- [ ] All Pascal charsets and Photoshop locales/versions.
- [ ] Layers without a unique native `lyid` and transformations
      beyond the two specific structural edits.
- [ ] Every unknown resource, EXIF, XMP or `shmd` semantic
      requirement and long-term metadata preservation.
- [ ] Different-length `luni`/Pascal source relocation.
- [ ] Reliable transactional customer output publisher and separate
      public release-policy approval.

**Production PSD mutation remains BLOCKED.**

## Final local regression

- [x] Combined M4c-3 through M4c-10 pinned Node.js 22.23.3 PSD
      research tests: **71 passed, 0 failed**.
- [x] Rust workspace `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed**, local macOS.
- [x] Rustfmt `cargo fmt --all -- --check`: passed.
- [x] Clippy `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] JavaScript source/test syntax and staged Git whitespace checks:
      passed.
- [x] Three machine-readable research receipts checked in offline
      mode without running native Photoshop again.
- [x] Native Photoshop 2026 remains responsive with **zero** open
      documents after research experiments.
