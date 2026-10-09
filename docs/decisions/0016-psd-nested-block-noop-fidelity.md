# ADR 0016 — M4c-4 PSD Nested Block and No-op Fidelity Gate

- **Status:** Accepted for research; production PSD mutation **BLOCKED**
- **Date:** 2026-10-09
- **Base:** `develop@0151a31e11aadb361505b1d87bbf24f2d4a434f1`
- **Protected baseline:** `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af`

## Decision

Continue the PSD safety research rather than advertising a working PSD
editor. M4c-4 extends ADR 0015's byte-level inventory to examine the
embedded `Layr`, `Lr16` and `Lr32` layer-record payloads, and records
the actual changes produced by a pinned `ag-psd` **no-op read/write/reopen**
across the committed conformance and benchmark corpus.

**A PSD that can be decoded, written and reopened may still lose raw
resources or change visible pixels.** A known four-character key, a zero
unknown-key count, or equal normalized layer metadata must never be used as
permission for a public save.

## Technical additions

The standalone research decoder
[`m4c4_nested_layer_inventory.cjs`](../../crates/yu-psd-spike/adapters/typescript/m4c4_nested_layer_inventory.cjs)
consumes the exact source bytes and verifies that the M4c-3 inventory is
bound to them by SHA-256. It recursively frames embedded layer records,
channel lengths/offsets/fingerprints, layer names, blend modes, flags and
layer Additional Layer Information. It rejects malformed bounds,
invalid signatures, unrepresentable 64-bit lengths, excessive layer or
tag counts and unexpected nonzero tail bytes. Nested tagged records are
bounded by depth and source section limits.

The representative corpus includes:

- a 16-bit PSD with `Lr16` holding **11 nested layer records**;
- a 32-bit PSB with `Lr32` holding **2 nested layer records**, including
  64-bit layer-channel and tag lengths;
- ordinary 8-bit PSD cases without embedded long layer records.

The tool does **not** decode compressed channel pixels, interpret opaque
nested metadata payloads or certify editability. It never modifies input.

The experimental
[`m4c4_noop_fidelity.cjs`](../../crates/yu-psd-spike/adapters/typescript/m4c4_noop_fidelity.cjs)
uses **Node.js 22.23.3** and **ag-psd 31.0.2**, with a bounded ImageData
factory (not a browser renderer), a fixed corpus and copied data only.
The writer produces throwaway candidate PSD/PSB files under ignored
`target/m4c4-psd-fidelity/run-*`, created exclusively without clobbering
any existing output. Input hashes are checked again after every attempt.
The probe deliberately loads linked-file content when the library can do
so, unlike M4c-1's lightweight skip-linked-data mode.

The byte-level comparator aligns resources by ID/name and occurrence,
additional layer blocks by location/key and occurrence, ordered layer
records, per-channel compressed bytes, nested layer records and composite
image payloads. It records **added**, **removed** and **changed** items
separately rather than calling all SHA-256 mismatches data loss.
It also compares the decoded composite and per-layer image pixels and
normalized layer attributes. A raw compression change may leave decoded
pixels intact; removal of an opaque image resource is still an important
unresolved fidelity failure even if the preview looks right.

## Reproducible local evidence

The machine results are committed as
[`m4c4-psd-noop-fidelity-v1.json`](../data/m4c4-psd-noop-fidelity-v1.json).

| Observation | Result | Interpretation |
|---|---:|---|
| Valid fixtures attempted | 13 | Committed PSD/PSB only |
| No-op save and reparse | **11** | Not proof of losslessness |
| Writer rejected high-bit input | **2** | 16-bit PSD and 32-bit PSB fail closed |
| Raw file differs on successful no-op | **11/11** | Re-encoding and/or metadata differences |
| At least one Image Resource removed | **9/11** | Opaque resource preservation not established |
| At least one Additional Layer Information block removed | **9/11** | Cannot promise metadata-preserving writes |
| At least one unknown item removed | **9/11** | Unsafe to broaden allow-list |
| Compressed composite byte differences | **8/11** | Encoding differences, not automatically visual changes |
| Decoded composite pixel mismatch | **1/11** | Confirmed pixel-level inconsistency on no-op |
| Fidelity certified | **0** | No production PSD writer authorized |

The **baseline-opacity** fixture is the case with a decoded composite
pixel mismatch. The advanced-blending fixture loses multiple Image
Resources and Additional Layer Information entries even though normalized
layer metadata and sampled decoded pixel fingerprints appear unchanged.
The Smart Object benchmark's tagged representation also changes:
`SoLE` tags are removed while `SoLd`/`PlLd` tags are added, with
linked-data handling still requiring independent validation.

The plain two-layer PSD and PSB fixtures are **not** preservation-certified:
even they show new/removed resources or different raw layer-channel
encodings. This narrows the optimism of M4c-1's four restricted rename
research candidates. All four remain *research fixtures only*, and
ADR 0014's exact-hash preflight continues to return
`write_authorized=false`.

## Important limits

The reported raw-block hashes measure serialized bytes. A changed channel
compression stream by itself does not prove the decoded pixel changed,
and a stable ag-psd decoded image does not prove Photoshop's full visual
or metadata semantics are preserved.

The 16-/32-bit nested blocks are **parsed read-only**, but the pinned writer
refuses high bit depth, so those nested structures cannot be compared
against a successful no-op output. The scanner may identify additional
known keys without understanding their nested semantic data.
No independent Photoshop save/open/render fidelity test has run.
Only the current local macOS test environment is verified.

## Next gate

**M4c-5 should be a targeted preservation/fidelity gate**, not a general
PSD editing CLI:

1. identify why common opaque resources, tagged metadata and Smart Object
   records disappear or change on no-op serialization;
2. assess whether a *minimal byte-preserving rename patch* is feasible
   without passing unsupported document sections through a general writer;
3. add independent cross-parser and Photoshop visual round-trip evidence
   on a broader source corpus;
4. keep visibility/opacity edits blocked until composited previews are
   recalculated or safely invalidated and verified;
5. continue to require source version binding, no-clobber publication and
   M4b-style post-write receipts for any future accepted mutation.

Until these gates pass, **no automatic general-purpose PSD writing, no
new `yu psd` mutation capability, and no v0.1 release baseline change**.
