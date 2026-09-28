# M3 Implementation Freeze Receipt

> Status: **Frozen implementation baseline; not a production release.**

YuTool now has a tested Managed PSD/PSB execution path, not just a candidate comparison. This receipt fixes the accepted scope, provenance and compatibility boundary. Changes after this baseline require their own validation; an existing freeze is not evidence that future builds passed.

## Baseline and provenance

| Item | Accepted value |
| --- | --- |
| Repository / CLI | `xiaodou997/yu-tool` / `yu` |
| M3 functional baseline | `c793279e3c131cae85adf73571590ab022b2ed05` |
| Functional tree | `3d42f311aa9d5cfca213ba6f852d9f9966767044` |
| Implementation PR | [#23](https://github.com/xiaodou997/yu-tool/pull/23), squash merged |
| Reviewed PR head | `8ab0339d3d1483885de17cc8c90e30424fc9f241` |
| Parent / PR #22 baseline | `87f0033af6a06094be523406578724f0a0327596` |
| Implementation CI checkout | `a611547fe04fed6498fbaab4a5bd9f5e78dc0eea` (GitHub test merge) |
| Freeze delivery | PR #24; documentation and regression guards only |
| Machine-readable receipt | [`m3-implementation-freeze-v1.json`](../data/m3-implementation-freeze-v1.json) |

The reviewed head and squash-merged baseline have identical trees, checked with `git diff --exit-code`. The freeze delivery commit is separate from the functional baseline; its exact merge SHA and checks belong in PR #24's delivery receipt to avoid a self-referential commit hash.

## Frozen capability scope

| Operation | M3 acceptance |
| --- | --- |
| `psd.inspect` | Document metadata |
| `psd.tree` | Canonical logical layer tree |
| `psd.layer.list` | Flattened canonical preorder |
| `psd.layer.info` | One layer by canonical ID |
| `psd.layer.export` | Selected stored 8-bit RGB bitmap to straight RGBA8/static PNG |

These capabilities are executable only when declared by the exact active Managed package. Inventory-wide unions of inactive versions do not authorize execution. Missing/broken/inactive engines fail explicitly. No implicit install, activation, upgrade, elevation or fallback is accepted.

```bash
yu psd inspect design.psd --json
yu psd tree design.psd --json
yu psd layer list design.psd --json
yu psd layer info design.psd --id L0002 --json
yu psd layer export design.psd --id L0002 -o layer.png --engine ag-psd --timeout-secs 30 --json
```

Names are presentation data; IDs use one-based logical preorder (`L0001`, `L0002`, ...), group before descendants. They are stable for a given logical tree, not persistent identifiers across arbitrary document reordering. The first four operations preserve the source. Export creates a new output and preserves the source as well.

## Engine and contracts

```text
engine/provider:    ag-psd / managed
package identity:   31.0.2+node22.23.3.yu2
upstream library:   ag-psd 31.0.2
private runtime:    Node.js 22.23.3
public JSON:        schema 1
engine manifest:    schema 1
external protocol:  version 1
PSD contract:       version 1
```

The Rust core and built-in raster operations do not acquire a system Node or Python dependency. M1 image info/resize and M2 installation, ownership, locking and discovery remain in place. The M3 additions to Manifest v1 fixed argv, active-version capability metadata and selected-engine error metadata are additive.

The preferred production direction remains ADR 0006: Managed ag-psd, with psd-tools as the independent reference/compatibility direction and rawpsd as experimental evidence. Only ag-psd is wired into the public PSD execution path. Discovery of another provider does not make it an executable fallback.

## Pixel and file contract

Export means stored layer-local pixels, including stored alpha, not a full-document render or a crop from the document composite. Cached text/shape/Smart Object bitmaps are not re-rendered. Masks, layer opacity, blend modes, effects, group composition and ICC transforms are not applied; successful exports carry this warning.

16/32-bit inputs, non-RGB modes, groups and missing/empty independent RGB bitmaps remain unsupported. A PSD can pass metadata inspection while its selected layer is unsupported for export.

The host resolves UTF-8 paths, requires an existing output parent and a new `.png` destination, and provides the engine only a private same-filesystem staging path. After Protocol v1 correlation, typed schema, ID and path validation, Rust decodes and validates a bounded static RGBA8 PNG (dimensions/data/CRC/trailer), syncs it, then creates the final hard link without overwriting. A racing destination is `OUTPUT_CONFLICT`; no unsafe copy fallback is allowed. Ordinary failures clean staging, while cleanup problems after successful publication are warnings. Power-loss durability and crash scavenging are not accepted claims.

Destination filesystems must support hard links. The installed engine is trusted code: process groups/Windows Job Objects provide lifecycle cleanup, not an operating-system sandbox against malicious engines or same-user directory replacement.

## Runtime bounds and errors

| Boundary | Value |
| --- | --- |
| Process deadline | 30 seconds default; explicit 1..3600 seconds |
| Request / stdout / stderr | 64 KiB / 16 MiB / 64 KiB |
| Export input | 512 MiB |
| RGBA bitmap | 256 MiB |
| Encoded PNG | 320 MiB |
| Windows quarantine retry | Up to two seconds for OS 5/32/33 only |

The export deadline is checked again before publication after validation. It does not interrupt individual filesystem or decoder calls. Combined memory can exceed a single buffer limit; no whole-process RSS bound is promised.

JSON success is one stdout envelope; JSON errors are one stderr envelope. Exit codes remain 0 success, 1 execution failure, 2 invalid input/argument/output conflict, 3 unsupported/unavailable/incompatible capability or engine. Paths are stdin JSON data and argv is fixed; no shell interpolation. `NODE_OPTIONS` and `NODE_PATH` are removed.

Windows removal retains the per-engine mutation lock. Only the same quarantine rename is retried; persistent errors preserve the original version. It never deletes/copies the original to hide a failed quarantine. The observed OS 5 failure did not identify its handle owner; no scanner/process attribution is asserted.

## Accepted implementation evidence

All rows below belong to the reviewed #23 head, not to the later freeze-document commit:

| Gate | Run | Result |
| --- | --- | --- |
| Standard CI | [99 / 36366437339](https://github.com/xiaodou997/yu-tool/actions/runs/36366437339) | Rustfmt and Ubuntu/macOS/Windows check, Clippy, tests passed |
| Managed package | [13 / 36366437379](https://github.com/xiaodou997/yu-tool/actions/runs/36366437379) | Three targets, lifecycle smoke, both actual CLI tests, artifact upload and manifest assembly passed |
| PSD Spike | [43 / 36366437367](https://github.com/xiaodou997/yu-tool/actions/runs/36366437367) | Actual standalone conformance/benchmark gates passed |

The real package matrix is Linux x86_64, macOS aarch64 and Windows x86_64. macOS x86_64, Windows ARM64, minimum OS versions and production signing are not established by these three hosted-runner labels.

Pixel acceptance covers three simple PSD/PSB/duplicate-name fixtures (two layers and 37,860 decoded RGBA bytes each), five representative RGB8 workloads matching frozen independent psd-tools fingerprints, and two high-bit rejections without output. Read-only coverage uses seven corpus entries. PNG container byte identity is not required; decoded pixel identity is.

The final implementation was also freshly tested locally on macOS ARM64: 113 ordinary tests passed, five optional-runtime tests ignored. That does not count as a local real-package test. The two real-package tests ran explicitly on the three hosted targets. See the [#23 acceptance receipt](https://github.com/xiaodou997/yu-tool/pull/23#issuecomment-5861807615) and [export checklist](../testing/pr23-psd-layer-export.md).

## Accepted package identities

[`ag-psd-managed-manifest-export-v2.json`](../data/ag-psd-managed-manifest-export-v2.json) preserves the JSON values emitted by the successful assembly job `108754451624`; only formatting is normalized. These are the **inner engine ZIP** hashes consumed by EngineInstaller:

| Target | Package SHA-256 |
| --- | --- |
| Linux x86_64 | `d3d420dde110f5774a7aba29ce3be1a6c41809b83c5732930c04b8c738b8bcb7` |
| macOS aarch64 | `cd1dc8caa0c8294114b39d8d5573e6d8647cd7aea09e094b073ad27afd7b406c` |
| Windows x86_64 | `07798e6b18de8d69902f5286d1dbccba557e59eb011625ae03f568ec0de1148c` |

Actions wraps uploads in separate ZIPs. Those artifact IDs/digests and the observed expiration (`2026-12-27T01:33:43Z`) are recorded separately in the machine-readable receipt. Artifact retention is not permanent hosting. Existing PR #21 four-capability receipts keep their original version `31.0.2+node22.23.3` and remain historical; they are not evidence for the `.yu2` export payload.

The placeholder `example.invalid` URLs are intentional. This snapshot is not a usable public installation endpoint. Rebuilding later does not inherit this receipt automatically: compare package hashes and rerun the real matrix; changed payloads need a new identity and new evidence.

## Freeze maintenance and release boundary

`crates/yu-cli/tests/m3_freeze.rs` guards the receipt's contracts, package metadata and historical evidence. It runs with ordinary `cargo test --workspace` without external runtimes or Git history. It checks consistency, not current online CI state and not actual pixel execution; the separate real-package gate remains required for runtime changes.

Keep this historical baseline and evidence unchanged when extending functionality. Use a new ADR/contract/receipt for changed semantics or package identity; update comparison guards explicitly rather than silently rewriting history. Additive capabilities do not justify regressions in existing commands. Reproduce with [PR #24 freeze checklist](../testing/pr24-m3-freeze.md).

M3 implementation freeze does not include a GUI, document mutation, rendering, raw `.rgba` file output, high-bit conversion, public engine catalog or release. The explicit next work should be release readiness, including durable accepted artifacts, trusted HTTPS manifests, project-license/redistribution decisions, CLI packaging acceptance, dependency lock/reproducibility policy and platform signing/notarization policy. The repository's current unlocked Cargo/npm resolution and hosted `latest` runners are not a whole-application reproducible-build guarantee.
