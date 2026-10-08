# M4c-1 — PSD Safe Mutation Feasibility (Experimental)

**Date:** 2026-10-08  
**Base:** `develop@9c9ea881585b7b7c1ec54f4b6c73fb507a480285`  
**Source / release boundary:** `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af` remains the frozen v0.1 technical candidate.  
**Decision:** research evidence accepted; **public PSD mutation remains BLOCKED**.

## 1. Question

Can YuTool safely modify a selected PSD/PSB layer's *name*, *visibility*
or *opacity* using the existing Managed-engine candidate `ag-psd`?

**A successful write plus successful reparse is necessary but insufficient.**
PSD writing may alter cached composites, embedded resources, effects or
layer data without the requested modification involving any of them.
This milestone must not expose a new `yu psd ... write` command or expand
the frozen M3 PSD capability contract.

## 2. Exact experiment

| Property | Evidence |
|---|---|
| Runtime | Node.js **22.23.3** (isolated, not system Node) |
| Library | `ag-psd 31.0.2` installed from the existing locked package |
| Host | macOS arm64 (local); **no Linux/Windows verification** |
| Corpus | 12 committed, redistributable PSD/PSB fixtures |
| Operations | no-op control, rename, visibility toggle, opacity = 0.37 |
| Tests | 12 × 4 = **48** trials, each re-reads its own persisted candidate |
| Source protection | buffer-only edits; trial outputs under ignored `target/m4c1-psd-mutation/`; original SHA-256 checked again on success and failure |
| Evidence | [M4c-1 machine-readable results](data/m4c1-psd-mutation-evidence-v1.json) |
| Harness | [Mutation probe](../crates/yu-psd-spike/adapters/typescript/m4c1_mutation_feasibility.cjs), [Node test](../crates/yu-psd-spike/adapters/typescript/m4c1_mutation_feasibility.test.cjs) |

The reader uses `useImageData: true` and `useRawThumbnail: true`.
A minimal in-memory ImageData factory is supplied: **no browser canvas,
flattening, or Photoshop-equivalent renderer is used**. Linked-file bytes
are deliberately skipped during this spike (`skipLinkedFilesData: true`),
so Smart Object files cannot be deemed safe by this result.
The probe limits fixture sizes to 8 MiB and image-data creation to
2 million pixels, and uses a Node heap budget.

The harness compares logical preorder layer identities (`L0001`, etc.),
parent/child structure, names, visibility, opacity (allowing PSD byte
quantization), bounds, blend mode, selected text/vector/effect/placed-layer
property digests, layer and mask RGBA hashes, cached composite pixel hash,
thumbnail hash and parsed image-resource keys/digests. This is **not**
a complete byte-for-byte audit of every opaque Photoshop block.

## 3. Observed results

| Evidence class | Count | Interpretation |
|---|---:|---|
| Persisted and reparsed | 40/48 | Writer can serialize these 8-bit fixtures, not proof of full fidelity |
| Rejected on write | 8/48 | All 16-/32-bit cases: writer rejects `bitsPerChannel` other than 8 |
| Requested mutation re-read correctly | 40/40 serialized | Target field is represented in the reparsed file |
| Unexpected layer/structure changes | 8/40 | Advanced blend and layer-effect fixtures change effect digests, **including no-op** |
| Image-resource changes | 4/40 | Advanced-blending resources change, including no-op |
| Cached composite pixel changes | 4/40 | Baseline-opacity sample changes on **no-op** and all three edits |
| Potential stale composite after visual edits | 18 trials | Visibility/opacity property changes are not followed by composite re-rendering |
| Restricted rename research candidates | 4 fixtures | Basic PSD/PSB, group rename and duplicate names pass measured invariants only |
| Source files preserved | 48/48 | Input SHA-256 unchanged after every attempt |
| Public mutation permitted | 0 | **Not authorized** by this research |

Selected fixture findings:

| Fixture | No-op / saved round trip | Rename | Visibility / opacity | Boundary |
|---|---|---|---|---|
| Simple 8-bit PSD | measured structure/composite preserved | narrow candidate | cached preview may be stale | no Photoshop fidelity evidence |
| Simple 8-bit PSB | measured structure/composite preserved | narrow candidate | cached preview may be stale | limited sample |
| Nested group | measured structure/composite preserved | **group** rename candidate | cached preview may be stale | group rendering not tested |
| Duplicate names | measured structure/composite preserved | `L0002` selected correctly | cached preview may be stale | never select by name |
| Text layer / masks | visible metadata and hashes pass measured checks | **not** accepted yet | cached preview may be stale | text/mask fidelity not independently certified |
| Smart Object | visible metadata checks pass | **not** accepted | cached preview may be stale | embedded linked-file bytes skipped; material completeness unproven |
| Advanced blending | **effect + resource changes** | blocked | blocked | unexpected non-target changes |
| Layer effects | **effect changes** | blocked | blocked | unexpected non-target changes |
| Baseline-opacity | **composite changes on no-op** | blocked | blocked | cannot preserve original visible composite |
| 16-bit PSD / 32-bit PSB | **write rejects** | blocked | blocked | no high-bit write support |

### Why visibility and opacity are still blocked

`ag-psd` explicitly documents that writing does **not** regenerate a
document's cached composite and thumbnail after layer appearance changes.
An unchanged cached composite after toggling visibility or changing opacity
does **not** count as successful visual preservation: it can mean that the
preview is **stale**. A correct metadata round trip must not be advertised
as a Photoshop-correct render.

### Why rename has only a restricted candidate status

For four small fixture classes, the selected name survives saving/reloading,
the observed layer structure is unchanged, and the measured composite and
resource fingerprints match. This supports an eventual *metadata-only*
candidate **behind a strict format/feature guard**, not a general PSD writer:
unknown additional-info blocks, linked Smart Object assets, ICC/profile
behavior, real Photoshop round-trip and broader files are not covered.
Serialized file bytes and lengths differ even on many no-op controls.

## 4. Feasibility decision

- **Rename:** conditional research candidate **only** for tightly restricted,
  independently verified 8-bit RGB documents; **do not expose CLI yet**.
- **Visibility:** metadata mutation/reparse works on tested inputs, but
  cached composite fidelity is unresolved; **BLOCKED**.
- **Opacity:** metadata mutation/reparse works with expected byte-level
  quantization, but cached composite fidelity is unresolved; **BLOCKED**.
- **Complex documents / effects / Smart Objects / high bit depth:** **BLOCKED**;
  no silent write fallback, no automatic normalization or feature stripping.
- **Safety:** never modify the source in place during a future experiment;
  use an isolated candidate, the version-bound output policy from M4b,
  a complete post-write receipt, and fail closed on any unexpected change.

## 5. Follow-up: M4c-2

Before implementing any public write command:

1. Freeze a **PSD mutation input/feature allow-list** (8-bit RGB, precise
   supported PSD/PSB header, bounded geometry, no unknown features) and
   create a strict preflight refusal contract. Unknown/opaque metadata
   must not be silently dropped.
2. Add broader fixtures and **independent Photoshop or equivalent
   application-level save/open/reopen evidence**, including thumbnails,
   layer effects, masks, layer order, and exact appearance checks.
3. For visibility/opacity, define composite recalculation or intentional
   cache invalidation plus preview behavior and validation before opening
   these operations. Do not confuse editor-managed preview refresh with
   pixel-accurate rendering by YuTool.
4. If rename-only reaches acceptable fidelity, design an **explicit**
   output-only modification proposal and dry-run with source SHA-256 binding,
   no-clobber, staged verification and an M4b-style receipt. Do **not**
   silently modify original PSD files.
5. Run independent macOS, Windows and Linux filesystem/lifecycle tests
   after CI quota is restored or local devices are available.

There is no PR, new PSD public command, production manifest, engine protocol
change, v0.1 tag or Apple notarization step in M4c-1.

## 6. Reproduce

From a clean checkout, install the already pinned npm dependencies locally:

```bash
npm ci --ignore-scripts --no-audit --no-fund --prefix packaging/ag-psd-engine
```

Use a **Node.js 22.23.3** executable (the local macOS probe used an isolated
`target/m4c1-node22/node_modules/node/bin/node`). Then:

```bash
NODE22=/path/to/node-v22.23.3
"$NODE22" --test crates/yu-psd-spike/adapters/typescript/m4c1_mutation_feasibility.test.cjs
```

The test executes all 48 trials and writes ignored candidate files plus
`target/m4c1-psd-mutation/evidence.json`. The committed report is a
macOS arm64 snapshot for comparison, not a universal expected result.
Its SHA-256 was
`74e47915b5fde46485b9022c3250b54277801b7a6564004f7f1049c25d30534e`
in two consecutive independent probe runs.
