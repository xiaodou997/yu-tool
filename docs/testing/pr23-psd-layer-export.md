# PR #23 — 8-bit PSD layer export acceptance

## Scope and baseline

PR #22 was squash merged at `87f0033af6a06094be523406578724f0a0327596`. This slice adds `psd.layer.export` without changing the v1 payload/result schema.

Export-capable engine package: `31.0.2+node22.23.3.yu2` (ag-psd 31.0.2, private Node 22.23.3). The old PR #21 manifest and receipts remain historical evidence for its four-capability package; they are not receipts for this new package. Package build hashes are emitted by the Managed Package workflow.

Supported: selected stored 8-bit RGB PSD/PSB bitmap → straight RGBA8, static PNG, layer-local dimensions. No mask/opacity/effect/blend application, group composition or ICC conversion. Cached text/shape/Smart Object pixels are not re-rendered. Groups, missing/empty RGB bitmaps, non-RGB modes and 16/32-bit inputs are unsupported.

## Public command

```bash
yu psd layer list design.psd --json
yu psd layer export design.psd --id L0002 -o layer.png --engine ag-psd --timeout-secs 30 --json
```

Prerequisites: explicitly installed and activated new package; existing destination directory on a filesystem supporting hard links. The destination must not exist and must end in `.png`. Placeholder prototype URLs are not a public distribution service.

The host writes through private same-filesystem staging, validates complete PNG decoding and response identity, and atomically creates the destination without clobbering it. Read the returned warnings and selected engine/version. Ordinary failed calls leave no published output; power-loss/crash cleanup is not covered.

## Ordinary Rust gate (no Node/Python required)

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

`yu-cli/tests/psd.rs` includes a native protocol fixture. The new regression cases cover exact RGBA8 bytes including transparent and semi-transparent pixels, final output identity, source preservation, existing outputs/source aliases, absent engine and old active package, mismatched layer/path/dimensions, invalid PNG, partial-write business failure, timeout, and staging cleanup.

`yu-runtime-psd` tests additionally cover CRC/trailer rejection, RGB versus RGBA rejection, dimensions and overflow bounds, missing output parent, wrong extension, a deterministic competing writer between preflight and publication, and Unix dangling symlinks.

## Real private-runtime gate

Run the Managed ag-psd Package workflow. On Linux x86_64, macOS aarch64 and Windows x86_64 it builds the verified `.yu2` archive and runs both tests in `psd_managed` with explicit `--ignored`.

Equivalent invocation after generating the local target package:

```bash
# Set YU_TEST_MANIFEST and YU_TEST_PACKAGE to absolute generated paths.
# macOS aarch64 example:
export YU_TEST_MANIFEST="$PWD/target/ag-psd-managed/macos-aarch64/manifest-macos-aarch64.json"
export YU_TEST_PACKAGE="$PWD/target/ag-psd-managed/macos-aarch64/yu-engine-ag-psd-31.0.2-node22.23.3.yu2-macos-aarch64.zip"
cargo test -p yu-cli --test psd_managed -- --ignored --nocapture
```

The new real export test deliberately empties the CLI's PATH and supplies unusable NODE_OPTIONS/NODE_PATH to prove private runtime selection and environment sanitization. It runs the actual CLI, not an adapter-only helper.

### Pixel acceptance

Simple PSD, corresponding PSB and duplicate-name derivative must each export two layers with a total of 37,860 decoded RGBA bytes and concatenated raw-pixel SHA-256:

```text
bea0c17a1c85d0dfcaa95c7bd5f0df6184e3bc48c81e56078130d39af4e62062
```

For the five 8-bit representative workloads, decoded PNG bytes, exported count and concatenated RGBA SHA-256 must match the independently recorded `psd-tools` rows in `docs/data/psd-benchmark-report-v2.json`. Do not regenerate expected hashes using the engine under test. PNG container bytes may differ while decoded pixels remain equal.

The 16-bit PSD and 32-bit PSB workloads must return `UNSUPPORTED_CAPABILITY` without output. The test also rejects group composition, nonexistent IDs and malformed files, verifies every source hash remains unchanged, rejects overwrites, and deactivates/removes the installed package.

## Evidence recording

Ordinary tests intentionally ignore optional-runtime tests. A green ordinary Rust gate alone is not evidence of real PSD export. Record exact feature head and completed workflow runs in the PR acceptance receipt, distinguishing local tests from GitHub-hosted package tests. The adapter changed in this slice, so standalone PSD Spike conformance must also be checked.

## Limits and non-goals

Input bound: 512 MiB. RGBA bitmap bound: 256 MiB. PNG artifact bound: 320 MiB. Combined runtime memory can be larger; no whole-process RSS guarantee is made. The process deadline is checked again before publication after verification, not during individual filesystem or decoder calls.

No overwrite flag, source modification, high-bit conversion, render fidelity, GUI, public engine catalog, signing or hosted production release is introduced. Filesystems without hard-link support fail closed rather than falling back to a non-atomic copy.
