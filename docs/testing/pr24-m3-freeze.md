# PR #24 — M3 implementation freeze checklist

## Identity

Functional baseline: `c793279e3c131cae85adf73571590ab022b2ed05` (PR #23 squash merge).
Reviewed implementation head: `8ab0339d3d1483885de17cc8c90e30424fc9f241`.
This PR adds documentation, evidence snapshots and offline regression guards; it does not change runtime behavior or rebuild an accepted package into a new identity.

Read [M3 Freeze](../milestones/m3-freeze.md) and [`m3-implementation-freeze-v1.json`](../data/m3-implementation-freeze-v1.json). The historical #23 CI evidence and the later freeze-PR checks must be reported separately.

## Source and ordinary checks

With both commits available:

```bash
git diff --exit-code 8ab0339d3d1483885de17cc8c90e30424fc9f241 c793279e3c131cae85adf73571590ab022b2ed05
git rev-parse 'c793279e3c131cae85adf73571590ab022b2ed05^{tree}'
# Expected tree: 3d42f311aa9d5cfca213ba6f852d9f9966767044
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test -p yu-cli --test m3_freeze
```

The three `m3_freeze` tests validate capability/contract consistency, target/package/provenance consistency and preservation of historical evidence/release boundaries. Historical text hashes normalize CRLF to LF for Windows checkout compatibility. They do not download packages, query GitHub, require full Git history, or execute the optional Node/Python runtimes.

Check that the reviewed freeze diff changes no production runtime, adapter, builder, fixture or dependency declarations. An added test is not a reason to silently change the functional baseline. Record the freeze head and actual completed CI run IDs in the PR delivery receipt; do not pre-fill success while checks are pending.

## Real package evidence

The accepted implementation matrix is Managed Package run `36366437379`, with explicit execution of both actual-CLI tests on Linux x86_64, macOS aarch64 and Windows x86_64. PR #23's separate Standard CI and PSD Spike runs are recorded in the freeze receipt. Later validation runs can support the freeze delivery, but do not replace these immutable source/package records.

To rerun actual package acceptance, use the builder and environment setup in [PR #23 export checklist](pr23-psd-layer-export.md), then:

```bash
cargo test -p yu-cli --test psd_managed -- --ignored --nocapture
```

Zero executed tests or only the ordinary gate is not real-package proof. Inspect for two passing tests, five independent RGB8 workload fingerprints, three simple PSD/PSB/duplicate-name fixtures, two high-bit rejections, source preservation and cleanup. The tests intentionally clear PATH and poison Node override variables to exercise private-runtime use.

## Package integrity and distribution

The accepted export manifest is [`ag-psd-managed-manifest-export-v2.json`](../data/ag-psd-managed-manifest-export-v2.json). Its values were recovered from the successful assembly job `108754451624`, not invented from artifact filenames. Compare SHA-256 of the inner engine ZIP to the manifest; the Actions artifact wrapper has a different digest.

Prototype `example.invalid` URLs are not installable public endpoints. The source receipt records artifact IDs and observed expiry; obtaining a future package must use trusted retained artifacts or a newly verified build. Do not upload a changed payload under the accepted package identity. Historical PR #21 manifests/receipts remain unchanged.

## Closeout

Record #23's actual merge SHA, #24's actual head/merge SHA, the ordinary test totals, completed workflow results, and a clean tracked worktree. No release tag, GUI, catalog, high-bit export, rendering, signing/notarization or durable public hosting is implied by this freeze. Do not mark future M4/M5 work complete.
