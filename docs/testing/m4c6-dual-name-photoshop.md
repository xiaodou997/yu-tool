# M4c-6 Acceptance — Dual PSD Layer Names and Independent Reader Fidelity

**Base:** `develop@07fdda430c1b6b9588e1d592785d2b7772fdf044`

**Branch:** `research/m4c6-dual-name-independent-fidelity`

## Technical acceptance

- [x] Pinned Node.js 22.23.3 with ag-psd 31.0.2.
- [x] Independent Python 3.12 environment with psd-tools 1.20.0.
- [x] Only M4c-2 four exact known input hashes, disposable copies under
      ignored target/.
- [x] Independent PSD logical preorder layer names and composite preview
      compared before/after source-preserving fixed-width Unicode patches.
- [x] Legacy Pascal / Unicode name inconsistencies reported as risk.
- [x] Plan-only longer Unicode names, explicitly not implementing writes.
- [x] Photoshop 2026 started and queried using its read-only scripting
      interface; all documents closed without saving.
- [x] Real Photoshop detected two background layers whose Unicode patch
      was **not** reflected in editor-visible names.
- [x] Photoshop group and duplicate non-background layer rename cases
      verified by reading layer names.
- [x] All originals and Photoshop disposable copies checked unchanged.
- [x] No new PSD mutation CLI, Managed engine protocol, public capability,
      CI/PR or v0.1 `main` branch change.

## Local reproducibility

```bash
npm ci --ignore-scripts --no-audit --no-fund --prefix packaging/ag-psd-engine
uv venv --python 3.12 target/m4c6-py312
uv pip install --python target/m4c6-py312/bin/python psd-tools==1.20.0

NODE22=target/m4c6-node22/node_modules/node/bin/node
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c6_dual_name_fidelity.cjs --check
"$NODE22" --test crates/yu-psd-spike/adapters/typescript/m4c6_dual_name_fidelity.test.cjs

# Explicitly invoked Mac desktop test — requires installed and accessible
# Adobe Photoshop 2026; never run as mandatory headless CI.
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c6_photoshop_readonly.cjs --check

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
git diff --check
```

The Photoshop application receipt reflects one local native run, not
multi-platform certification. The automated test checks the committed
receipt; re-running Photoshop itself requires GUI/app automation access.

## Results

- [x] Independent cross-parser name alignment: 4/4.
- [x] psd-tools independent preview hash equality: 4/4.
- [x] Real Photoshop name read: 2/4 pass, **2/4 fail for Background layer**.
- [x] Legacy-name divergence: 3/4 after Unicode patch.
- [x] Different-length name plans: 4, **no actual writes allowed**.
- [x] Pinned cross-parser Node.js suite: **5 passed, 0 failed**.
- [x] Repeatable `--check` of independent reader and native Photoshop receipts.
- [x] Rust workspace `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** (local macOS).
- [x] Rustfmt `cargo fmt --all -- --check`: passed.
- [x] Clippy `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] Node.js source/test and Python AST syntax: passed.
- [x] Git whitespace check: passed.
- [x] Photoshop's document count after the read-only experiments: **0**.

## Still not authorized

- [ ] Real Photoshop document save/reopen and visual/composite fidelity.
- [ ] Non-background non-ASCII name with matching legacy encoding.
- [ ] Safe edit of native Photoshop Background layer.
- [ ] Variable-length `luni` structural relocation and original metadata
      preservation.
- [ ] Arbitrary customer PSD/PSB and independent Windows/Linux verification.

**Public PSD mutation remains BLOCKED.** See
[ADR 0018](../decisions/0018-psd-dual-name-photoshop-acceptance.md).
