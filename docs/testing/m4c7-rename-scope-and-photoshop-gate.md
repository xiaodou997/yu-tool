# M4c-7 Scope Freeze and Native Photoshop Save Gate — Acceptance

**Research base:** `develop@d197334c88d123b4a2a68600ea784c543b41755b`

**Protected v0.1 main:** `5526dc4465c4e79ad7f2757d06b3b1693642b8af`

## Frozen boundaries

- [x] Exact SHA-bound existing four research fixtures only.
- [x] Native Photoshop Background layer is denied.
- [x] Only vetted pixel/group kinds are considered; actual
      save/reopen research requires the specific exact-hash case.
- [x] Only same-length NFC UTF-16BE rename proposals, with matching
      old/new legacy Pascal ASCII length and original Unicode equality.
- [x] Raw layer index, canonical ID and expected old name checked.
- [x] Old resource/channel/composite bytes remain protected; production
      writes always denied.
- [x] Research-only Photoshop saveAs uses distinct scratch inputs,
      no-clobber output path checks and a native responsiveness preflight.
- [x] Separate *unchanged original-save control* to distinguish baseline
      Photoshop serializer effects from proposed rename-specific effects.
- [x] Original fixture SHA-256 must remain unchanged in every outcome.

## Evidence

[Frozen machine policy](../data/psd-rename-scope-m4c7-v1.json)

[Deterministic scope receipt](../data/m4c7-rename-scope-evidence-v1.json)

[Native Photoshop save experiment receipt](../data/m4c7-photoshop-control-v1.json)

Local observation:

- Four known research documents examined: **1** isolated research
  candidate; **3** explicitly blocked.
- Photoshop's AppleScript read-only document count and test-open channel
  timed out in this run. Save-and-reopen is **not completed**.
- The fast-fail guarded experiment reports a native-app blocker,
  zero saved control/candidate outputs and zero production permissions.
- No Photoshop restart, no forced process termination and no original PSD
  replacement was attempted.

## Reproduce

```bash
NODE22=target/m4c7-node22/node_modules/node/bin/node
"$NODE22" --test crates/yu-psd-spike/adapters/typescript/m4c7_scope_freeze.test.cjs
node -e "console.log(require('./crates/yu-psd-spike/adapters/typescript/m4c7_scope_freeze.cjs').buildReport())"

# Native Photoshop-only optional study; run explicitly when Photoshop and
# AppleScript are responsive. Uses Python 3.12 + psd-tools 1.20.0.
M4C6_PYTHON=target/m4c7-py312/bin/python "$NODE22" \
  crates/yu-psd-spike/adapters/typescript/m4c7_photoshop_save_probe.cjs --check

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
```

- [x] Node.js 22 scope/refusal suite: **5 passed, 0 failed**.
- [x] Full Rust workspace `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** (local macOS).
- [x] Clippy `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] Rustfmt `cargo fmt --all -- --check`: passed.
- [x] JavaScript syntax and Git whitespace checks: passed.

## Still blocked

- [ ] Two matching Photoshop saveAs/control outputs reopened and compared.
- [ ] Independent preview/mask/ICC/Smart Object/editor fidelity receipts.
- [ ] New normal pixel/non-ASCII/duplicate-name fixture with consistent
      legacy/Unicode representations.
- [ ] Variable-length name relocation and Windows/Linux conformance.
- [ ] Any public `yu psd layer rename` or PSD write permission.

**M4c-7a scope can freeze without M4c-7b/c becoming green.**
