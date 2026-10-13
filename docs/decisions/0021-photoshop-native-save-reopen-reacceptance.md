# ADR 0021 — M4c-7c Photoshop Native Save/Reopen Reacceptance

**Date:** 2026-10-13

**Base:** `develop@e0c8af6481ae4c3b86b80485ac14f510bedcfea0`

**Status:** Native **name persistence verified**; full PSD **fidelity BLOCKED**

**v0.1 technical candidate:** `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af` unchanged

## Scope and recovery

Following a user-initiated restart of **Adobe Photoshop 2026**, the
native readiness probe returned `ready_empty_workspace`, with
`open_documents=0`. M4c-7a's unchanged admission policy allowed only the
exact-hash, dual-name-synchronized `nested-group` research fixture;
both native Background examples and the legacy-inconsistent duplicate
name example remain prohibited.

M4c-7c then performed the previously blocked **paired native editor
saveAs/close/reopen experiment** on disposable PSDs in ignored
`target/m4c7-psd-photoshop-save/run-*`. The committed source and two
experiment input copies were verified unchanged by SHA-256.
The program never saved a user document, used the same destination twice,
or published a PSD into the product runtime.

## Native result: one limited capability now proved

| Validation | Outcome |
|---|---|
| Photoshop Apple Events readiness and empty document set | PASS |
| Photoshop saveAs of unmodified control into new output | PASS |
| Photoshop saveAs of byte-patched group rename into new output | PASS |
| Photoshop close-and-reopen original control group `Group 1` | PASS |
| Photoshop close-and-reopen patched group `Group 2` | PASS |
| Other logical layer names and tree preserved on reopening | PASS |
| Independent `psd-tools 1.20.0` preview of both *saved* outputs identical | PASS |
| Layer channel compressed hashes in paired saved outputs identical | PASS |
| Saved compressed composite image bytes in paired outputs identical | PASS |
| `psd-tools` pre-save vs post-save composite preview bytes equal | **FAIL on both inputs** |
| All saved output bytes outside target name ranges identical | **FAIL** |
| Resource and additional metadata blocks in saved pair identical | **FAIL** |
| Product PSD mutation/write authorized | **NO** |

The two saved outputs have different byte lengths in this receipt:
the Photoshop-saved control is **33,256 bytes**, the renamed candidate is
**33,240 bytes**. The independently decoded RGBA previews of the *two
saved outputs* match. The independent source-vs-save preview changes on
both paths, including the unchanged control. It is **not established**
whether this represents normal Photoshop recomposition of a stale source
preview or another editor behavior; do not label it as damage caused by
the rename without further validation.

The raw scanner was conservatively extended to recognize and flag
1–3 **trailing zero alignment bytes** in Additional Layer Information,
as found on the new Photoshop outputs. This does not authorize writing
or deem other unknown blocks preserved. Synthetic malformed trailing
data must still fail closed.

## Residual source-fidelity blockers

The strict saved-control vs saved-candidate comparison shows:

- Identical original-image headers, raw layer records/properties, channel
  data and compressed merged-composite data.
- Correct dual-name `Group 1 → Group 2` in both Unicode `luni` and
  legacy Pascal representations.
- Distinct serialized Image Resources with IDs **1060**, **1050** and
  **1058**, plus several changed `shmd` tagged block payloads.
- Different final PSD file sizes. The comparator's exact byte-range gate
  therefore rejects the saved pair; full arbitrary resource fidelity is
  not proven.

**Additional baseline finding:** two *separate* Photoshop save runs of the
unchanged original exhibit changed raw fingerprints for resource IDs
`1060` and `1058` and several `shmd` blocks, without changing their
compressed composite image bytes. This is evidence that some metadata
changes can be produced by repeated Photoshop saving itself; it does
**not** explain every resource change (`1050`) or the 16-byte
control/candidate length difference. Do not automatically ignore these
fields in production.

## Decision and next gate

**M4c-7c functional Photoshop name-save/reopen verification is complete
for one exact, known, simple group fixture. M4c-7d/e full fidelity and
release approval remain BLOCKED.**

The source's byte-level fixed-width name patch and native Photoshop name
persistence are now supported by three types of evidence: raw bytes,
independent psd-tools, and a real Photoshop save/reopen. That still does
not justify `yu psd layer rename` as a public operation, arbitrary
input/rename length, Background rename, or visual/mask/ICC/Smart Object
preservation promises.

Recommended next step: M4c-7d should classify Photoshop's own
re-serialization metadata and preview recomposition using matched
untouched-control runs and independent native pixel/metadata checks,
without weakening strict unknown-block safety gates or prematurely
adding a PSD writer.

The [new v3 native receipt](../data/m4c7c-photoshop-native-reacceptance-v3.json)
is a local native snapshot and may contain nondeterministic Photoshop
metadata hashes. It is intentionally stored **separately** from the
[old v2 blocked receipt](../data/m4c7c-photoshop-control-v2.json).
`--write` explicitly runs native Photoshop; `--check` **only checks
the existing snapshot offline**, never silently performing saves.

See [acceptance](../testing/m4c7c-native-reacceptance.md).
