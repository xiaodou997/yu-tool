# M4c-11 — PSD Metadata Gate and Safe Output Research Acceptance

**Base:** `develop@5b20e4c3c196ac5933305bf6257e2c00df5ab137`

**Frozen `main`:**
`5526dc4465c4e79ad7f2757d06b3b1693642b8af`

**Result:** research-only output safety accepted, general PSD writes
remain **BLOCKED**.

## Scope

- [x] Reuse previous M4c-10 Photoshop-saved original/candidate files
      using exact SHA-256 binding; no new Photoshop save required.
- [x] Bounded TIFF EXIF inspection of **3 IFD / 22 tags** in both
      pairs; report tag `306:DateTime` as changed, without normalizing
      it or declaring all metadata equivalent.
- [x] Fingerprint XMP `DocumentID`, `InstanceID`, `History`,
      `ModifyDate`, `MetadataDate` as provenance fields.
- [x] Slices name difference explained for the exact saved files,
      with the remainder of the resource equal.
- [x] Check raw `shmd/cust` entries and preserve unknown semantics.
- [x] Standalone private output routine accepts only the frozen
      8-bit RGB M4c-8 exact-hash PSD and deterministic same-width
      `Layer 1 → Layer A` patch.
- [x] Research output stage has exclusive creation and file fsync;
      same-filesystem hard-link publication never replaces an
      existing destination.
- [x] Prepublication source inode/hash recheck; postpublication
      output hash and structural verification.
- [x] Negative tests cover existing/symlink output, racing output,
      changed/replaced/symlink source, bad source SHA and unsafe paths.
- [x] Source and original fixtures remain unchanged, with no
      production CLI / writer permission changes.

## Commands

```bash
NODE22=target/m4c11-node22/node_modules/node/bin/node
PYTHON=target/m4c11-py312/bin/python

# Native saved sources remain from M4c-10's prior ignored scratch run.
# Strict read-only snapshot collection:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c11_metadata_semantic_gate.cjs \
  --write /absolute/path/to/m4c10-multilingual-sync/run-...

# Only fixed known input/output under target; no Photoshop save:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c11_safe_output_spike.cjs --write

# Offline-only snapshots, no source PSD mutation:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c11_metadata_semantic_gate.cjs --check
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c11_safe_output_spike.cjs --check

"$NODE22" --test \
  crates/yu-psd-spike/adapters/typescript/m4c11_metadata_semantic_gate.test.cjs \
  crates/yu-psd-spike/adapters/typescript/m4c11_safe_output_spike.test.cjs

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
```

- [x] Targeted M4c-11 Node suite: **12 passed, 0 failed**.
- [x] Complete M4c-3 through M4c-11 pinned Node.js 22.23.3 PSD suite:
      **83 passed, 0 failed**.
- [x] Rust `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** on local macOS.
- [x] Rust `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] Rustfmt, JavaScript syntax, both offline evidence checks and
      Git whitespace checks: passed.

## Remaining release blockers

- [ ] Unverified arbitrary XMP/EXIF/`shmd`/opaque resources must
      continue to block a generic mutation writer.
- [ ] Source-content race after last check and directory durability,
      cross-filesystem portability, post-publication ambiguity.
- [ ] Real-world PSD feature diversity and Photoshop native output
      fidelity, especially Background, Smart Object, masks/effects,
      encoding changes and variable-length layer names.
- [ ] Explicit, separate manual production release decision.

**Public PSD mutation remains `BLOCKED`.**
