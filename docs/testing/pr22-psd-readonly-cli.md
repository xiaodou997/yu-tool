# PR #22 — PSD Runtime Wiring / Read-only CLI

## Scope

Base: PR #21, `c324d256ef19b22d876eb29168b1a4167cf9fb2c`.

This change exposes `psd.inspect`, `psd.tree`, `psd.layer.list`, and `psd.layer.info` through the activated Managed ag-psd package. It does not expose export, render, mutation, a GUI, a catalog, or public package hosting. Package provenance, adapter versions, and the M2 install/activation separation remain unchanged.

## Standard automated gate

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

The normal Rust gate does not need Node or Python. `crates/yu-cli/tests/psd.rs` compiles a small Rust-only test engine with `rustc`, installs it through the real checksum-verifying installer, and exercises the public binary. The real-package test is explicitly ignored in this gate, not silently substituted with the fixture engine.

Expected coverage:

- four read-only commands and human/JSON output;
- schema-v1 result and optional engine identity on errors;
- no implicit install, activation, or alternative engine fallback;
- exact active-version capabilities rather than installed-version union;
- absent/inactive/broken engine state;
- canonical layer IDs, invalid/missing selectors, and unmatched result IDs;
- malformed JSON, multiple JSON responses, invalid UTF-8 response, mismatched request ID, protocol/contract version mismatch, invalid result shape, non-zero exit, and valid business errors;
- stdout/stderr limits and timeout cleanup for a silent process or ordinary descendant retaining pipe handles;
- relative paths/data roots, spaces/non-ASCII/metacharacter filenames, and rejection of non-UTF-8 paths without lossy substitution;
- input hashes unchanged and explicit lifecycle cleanup.

## Real private-runtime CLI gate

Run on the current supported target (Linux x86_64, macOS aarch64, or Windows x86_64). Replace `<os>`, `<arch>`, and `<label>` with one matching row:

| OS | Arch | Label |
| --- | --- | --- |
| linux | x86_64 | linux-x86_64 |
| macos | aarch64 | macos-aarch64 |
| windows | x86_64 | windows-x86_64 |

Install the exact build-only dependencies in `packaging/ag-psd-engine` with `npm install --ignore-scripts --no-audit --no-fund`, then return to the repository root:

```bash
python3 tools/build_ag_psd_managed_package.py build \
  --sources packaging/ag-psd-engine/node-runtime-sources-v1.json \
  --target-os <os> --target-arch <arch> \
  --node-modules packaging/ag-psd-engine/node_modules \
  --protocol-adapter crates/yu-psd-spike/adapters/typescript/ag_psd_protocol.cjs \
  --output-dir target/ag-psd-managed/<label> \
  --package-base-url https://example.invalid/yu-tool/ag-psd-engine-prototype-v1
```

Set `YU_TEST_MANIFEST` to the generated `manifest-<label>.json` and `YU_TEST_PACKAGE` to `yu-engine-ag-psd-31.0.2-node22.23.3-<label>.zip`, using absolute paths. Then:

```bash
cargo test -p yu-cli --test psd_managed -- --ignored --nocapture
```

This test requires both inputs and fails if they are absent. It verifies the real package SHA-256 via a test-only local downloader, explicitly activates the version, and invokes the real `yu` binary with an empty `PATH` and deliberately unusable `NODE_OPTIONS`/`NODE_PATH`. It covers all seven committed corpus entries: six accepted PSD/PSB fixtures through inspect/tree/list and every layer through info, plus explicit malformed-input rejection. It also checks duplicate-name IDs, selected engine/version, unavailable alternate engine, nonexistent layer ID, source hashes, deactivation, and removal.

The `Managed ag-psd Package` workflow runs this test in every platform leg after the original PR #21 lifecycle smoke. Standard `CI` still runs the full native regression suite separately.

## Manual usability check

Use an explicitly installed/activated trusted package with a real download location. Prototype manifests still have placeholder URLs; do not expect the public install command to download from them.

```bash
yu capabilities --json
yu psd inspect design.psd --json
yu psd tree design.psd
yu psd layer list design.psd --json
yu psd layer info design.psd --id L0001 --engine ag-psd --json
```

Check readable tree output, unambiguous IDs, real engine version in JSON, and source file preservation. This checklist does not claim broad Photoshop render fidelity or large-production-document acceptance; neither belongs to PR #22.

## Evidence recording

Record the validated commit, OS/architecture, exact commands, and workflow run URLs in the PR acceptance receipt. A green ordinary Rust gate does not imply that the ignored real-package test ran. Do not mark cross-platform acceptance complete until all three real-package legs finish successfully.
