# ADR 0023 — M4c-7e PSD Mutation Research Exit Decision

**Date:** 2026-10-13

**Decision:** conclude the M4c-7 research increment; **do not enable PSD writing**.

## Evidence and outcome

The exact-hash Photoshop group rename `Group 1 → Group 2` is
persisted through real Photoshop 2026 save/reopen. Two independent
paired control runs show identical native Photoshop PNG rendering
before and after saves and identical saved-control vs saved-candidate
pixels. The independent `psd-tools` original-preview mismatch
(160 pixels, max channel delta 11) was not a change in Photoshop-native
rendering of this exact fixture.

This establishes a narrow technical feasibility result for
**fixed-width Unicode + matching ASCII Pascal name-byte patches**.
It is not evidence that arbitrary PSD metadata and editor semantics
remain intact.

M4c-7d's metadata forensics identified Image Resource 1050 as Slices,
and explained the 16-byte difference between independently saved
control/candidate files as different serialized Slices document names
`nested-group-control` and `nested-group`. The surrounding resource
bytes match for these exact files. This must **not** become a general
resource ignore-list.

Remaining uncertain classes include EXIF (1058), XMP (1060)
document-instance/history fields, multiple 52-byte per-layer
`shmd/cust` records, and other opaque Photoshop resources.
Native saving itself introduces some dynamic metadata changes.
Their semantic safety is not proven.

## Extended corpus review

All **13 valid frozen PSD/PSB fixtures** were inspected read-only
using the bounded raw scanner and independent `psd-tools 1.20.0`
layer-kind parser. A further **3 fixtures** were actually opened and
closed without saving in Adobe Photoshop 2026:

| Research fixture | Native observation | Admission decision |
|---|---|---|
| `bench-baseline-rgb` | Two ordinary pixel layers, no native Background | Candidate for a **new exact-hash research experiment**, *not* an approved writer input |
| `simple-pixel-layers-psd` | Ordinary Cyrillic layer plus localized `背景` native Background | No Background edits; legacy encoding compatibility needs separate tests |
| `text-layer` | Type layer `A` readable in Photoshop | Excluded from mutation |

The current M4c-2/M4c-7a exact-input research admission is **not
broadened**. One viable pixel fixture is only eligible for future
review, not a current modification authorization.

## Frozen milestone decision

- M4c-7c native group rename save/reopen: **research PASS**.
- M4c-7d limited Photoshop-native RGB preview preservation for
  this one fixture: **research PASS**.
- M4c-7d Slices 1050 file-name difference: **explained for fixture only**.
- Metadata `shmd`/EXIF/XMP full semantics: **BLOCKED**.
- Broader input, multilingual, Background, high-bit, text, effects,
  Smart Object, different-length names: **BLOCKED**.
- PSD production mutation/write, `yu psd rename` and unrestricted
  save: **BLOCKED**.

The [machine-readable decision](../data/m4c7e-release-decision-v1.json)
must remain `public_psd_write_authorized=false`.
The v0.1 technical release baseline on `main` stays frozen;
already supported read-only PSD and raster operations are not
implicitly blocked by this separate PSD mutation research decision.

## Safe next work

Continue with a separate evidence-focused research increment, not
release-only PR. Add new exact-hash normal pixel and non-ASCII
Unicode/legacy-name fixtures, perform paired native Photoshop
save/reopen comparisons, investigate dynamic EXIF/XMP/`shmd` fields,
and retain strict deny-by-default opaque block handling. Only after
independent safety evidence and a separate release-policy decision
should a version-bound, new-output-only rename capability be
considered.

See [M4c-7d ADR 0022](0022-photoshop-m4c7d-metadata-fidelity-boundary.md),
[read-only corpus profile](../data/m4c7e-readonly-corpus-profile-v1.json),
[native read-only receipt](../data/m4c7e-native-readonly-samples-v1.json),
and [acceptance](../testing/m4c7e-research-exit-acceptance.md).
