# M4c-7c — Native Photoshop Readiness and Saved-Pair Research Acceptance

**Base:** `develop@930e1d83068926130a22274f618390e70c5f52f8`

**v0.1 `main`:** `5526dc4465c4e79ad7f2757d06b3b1693642b8af` unchanged.

## Code and evidence gates

- [x] Refuse if Photoshop is absent, Apple Events times out, automation
      is denied, unexpected output is returned, or user documents are open.
- [x] Preflight repeats immediately before each native saveAs.
- [x] Do not activate, restart, force-quit, manipulate or save user's
      existing Photoshop documents.
- [x] Saved-pair research gate checks raw resource/tag/channel/merged-image
      equality and every changed byte is within verified `luni` and
      Pascal target-name ranges.
- [x] Non-target metadata, pixel-channel and resource mutations
      fail closed; strict comparison does not grant product authorization.
- [x] Experimental outputs stay in new ignored `target/` directories.
      Existing output path checks do not claim atomic no-clobber.
- [x] A failed or timed-out native write is never reported as a
      successfully saved/reopened PSD; preserve failure artifacts.
- [x] M4c-7a admission still allows only the frozen exact-hash group
      *for research* and never authorizes a public PSD mutation.

## Current native observation

The current local Adobe Photoshop 2026 process is running but its
AppleScript `count documents` command times out. A separate System
Events window query is denied Accessibility permission to `osascript`,
so the presence of an in-app modal dialog is unconfirmed.

The [M4c-7c native receipt](../data/m4c7c-photoshop-control-v2.json)
records one admitted research fixture, **zero saves attempted**,
**zero paired save/reopen passes**, and **zero production writes**.
No test claims Adobe application fidelity has passed.

## Local checks

```bash
NODE22=target/m4c7c-node22/node_modules/node/bin/node
"$NODE22" --test \
  crates/yu-psd-spike/adapters/typescript/m4c7_scope_freeze.test.cjs \
  crates/yu-psd-spike/adapters/typescript/m4c7c_native_readiness.test.cjs \
  crates/yu-psd-spike/adapters/typescript/m4c7c_saved_pair_gate.test.cjs

# Manually invoked only; bounded native preflight; no save if unresponsive.
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c7_photoshop_save_probe.cjs --check

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
git diff --check
```

- [x] Full pinned Node 22.23.3 scoped suite: **15 passed, 0 failed**.
- [x] Rust serial workspace `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** (local macOS).
- [x] Clippy `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] Rustfmt `cargo fmt --all -- --check`: passed.
- [x] JavaScript source syntax and Git whitespace checks: passed.

## Blocking gate for production

- [ ] Native Photoshop 2026 responsive with zero user documents.
- [ ] Exact group control and edited document both saved separately,
      reopened and independently compared in Photoshop and psd-tools.
- [ ] Original fixture remains byte-identical; control/candidate data and
      visual/metadata evidence independently certified.
- [ ] Windows/Linux replay and production-safe versioned publication.

**M4c-7c remains BLOCKED until these checks are completed.**
