# M4c-1 — PSD Mutation Feasibility Acceptance

**Branch:** `spike/m4c1-psd-safe-mutation-feasibility`  
**Base:** `develop@9c9ea881585b7b7c1ec54f4b6c73fb507a480285`  
**Scope:** investigation only; no stable `yu psd` write capability.

## Acceptance and limits

- [x] Use pinned ag-psd **31.0.2** and Node **22.23.3**.
- [x] Use only committed, redistributable PSD/PSB fixtures.
- [x] Keep all trial files under ignored `target/`; recheck source SHA-256.
- [x] Attempt 12 fixtures × 4 operations (no-op, rename, visibility, opacity).
- [x] Save candidate to disk and reparse; compare requested metadata.
- [x] Compare hierarchy, layer/mask pixels, effects, resource digests,
      thumbnails and cached composite fingerprints where observable.
- [x] Confirm supported 8-bit saves are distinguishable from explicit
      high-bit-depth writer rejection.
- [x] Record actual incompatibilities without labeling passing parses safe.
- [x] Exercise group rename and duplicate-name selection by canonical ID.
- [x] Machine-readable evidence snapshot and risk decision committed.
- [x] No new production PSD mutation operation or external protocol change.
- [x] No `main`, tag, signing, notarization, public release or PR change.

## Commands run locally on macOS arm64

```bash
npm ci --ignore-scripts --no-audit --no-fund --prefix packaging/ag-psd-engine
# A pinned Node.js 22.23.3 was installed only under ignored target/.
target/m4c1-node22/node_modules/node/bin/node --test \
  crates/yu-psd-spike/adapters/typescript/m4c1_mutation_feasibility.test.cjs
```

- [x] Pinned Node test: **1 passed, 0 failed**, exercising **48 trials**.
- [x] Repeated independent probe yielded identical evidence SHA-256.
- [x] `cargo test --workspace --locked`: **174 passed, 0 failed**
      (existing Rust workspace contract remains unchanged).
- [x] `cargo fmt --all -- --check`: PASS.
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- [x] JavaScript syntax and `git diff --check`: PASS.
- [ ] Independently replay the fixture matrix on Windows and Linux.
- [ ] Open/save results in Photoshop or another independent full-fidelity
      editor and verify visual/output/embedded-resource preservation.
- [ ] Decide an explicit composite/thumbnail update or invalidation policy.
- [ ] Validate unknown/opaque Photoshop additional-info preservation.

The unchecked items block public PSD mutation. They are **not**
requirements for completing the experimental M4c-1 research slice.

See [findings and decision](../psd-mutation-feasibility-m4c1.md) and
[machine evidence](../data/m4c1-psd-mutation-evidence-v1.json).
