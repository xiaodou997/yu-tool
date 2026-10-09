# ADR 0018 — M4c-6 PSD Dual Names and Photoshop Read-only Acceptance

- **Status:** Research complete; PSD mutation remains **BLOCKED**
- **Date:** 2026-10-10
- **Starting base:** `develop@07fdda430c1b6b9588e1d592785d2b7772fdf044`
- **Frozen v0.1:** `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af` unchanged

## Decision

**Accept the controlled, fixed-width byte-patch research as promising,
but do not offer general PSD layer rename, save, visibility or opacity
commands.** M4c-6 produces independent parser results and a real Adobe
Photoshop 2026 *read-only name receipt*. They reveal a real compatibility
failure not visible to `ag-psd` alone.

## Two independent validation layers

The [cross-parser experiment](../../crates/yu-psd-spike/adapters/typescript/m4c6_dual_name_fidelity.cjs)
uses Node.js **22.23.3**, `ag-psd 31.0.2` and an isolated Python
**3.12 / psd-tools 1.20.0** environment. It creates new disposable copies
under ignored `target/` using M4c-5's exact four-hash research allow-list
and fixed-width `luni` patches. It reopens originals and candidates with
an **independent parser**, comparing canonical logical tree names, targeted
layer identity and `psd-tools` decoded RGBA composite preview hashes.

Results on local macOS:

| Observation | Evidence |
|---|---:|
| Cross-parser selected Unicode name and logical-tree changes agree | **4/4** |
| psd-tools decoded composite preview hash before/after agrees | **4/4** |
| Legacy Pascal name may differ from newly patched Unicode name | **3/4** |
| Source SHA-256 remains unchanged and unrelated raw bytes preserved | **4/4** |

The `psd-tools` preview is **not** a Photoshop visual rendering oracle.
It is independent corroboration from a different parser, not a sign-off
on native Photoshop fidelity or PSD saving.

The [Photoshop read-only adapter](../../crates/yu-psd-spike/adapters/typescript/m4c6_photoshop_readonly.cjs)
was run on a macOS machine with **Adobe Photoshop 2026** installed. It
opens only *new throwaway copies*, reads actual document layer names and
`isBackgroundLayer` / group status via AppleScript `do javascript`,
then always closes with `SaveOptions.DONOTSAVECHANGES`. It rechecks file
SHA-256 afterward and never initiates PSD saving.

| Controlled case | Photoshop name-read result | Finding |
|---|---|---|
| Group `Group 1` → `Group 2` | **PASS** | Photoshop exposes renamed group |
| Duplicate-name layer `X` → `Y` | **PASS** | Photoshop exposes renamed non-background layer |
| PSD background `Фон` → `Дом` | **FAIL** | Photoshop exposes localized `背景` both before and after |
| PSB background `Фон` → `Дом` | **FAIL** | Same localized special-background behavior |

Actual Photoshop background-layer identification, not just an assumption
from missing text, is recorded in the
[native name receipt](../data/m4c6-photoshop-name-receipt-v1.json).
Only **2/4** Photoshop layer-name cases pass. No test saved a Photoshop
document, and none proved full native visual/render/metadata fidelity.

**Conclusion:** a successful `luni` edit can be ignored or overridden by
Photoshop for its native Background layer, regardless of successful
`psd-tools` / `ag-psd` name reads. A production rename must reject this
class until a separate, editor-validated handling rule exists. Do not
silently convert a background to a normal layer to bypass the issue.

## Variable-length rename scope

The research adds a **plan-only** assessment for UTF-16BE name changes
with a different number of code units:

- the `luni` payload length would change;
- the enclosing per-layer extra-data length would change;
- the Layer Info section and enclosing Layer-and-Mask section lengths
  would need recalculation (32-bit PSD and/or 64-bit PSB fields);
- downstream byte offsets would shift;
- any legacy Pascal name changes have separate length, encoding and
  four-byte-alignment constraints.

These are **incomplete planning estimates**. M4c-6 does **not** perform
relocation, recompression, general Photoshop serialization or a
variable-length byte patch. Every plan returns
`proposed_write_authorized=false` and
`status=plan_only_not_implemented`. The plan is not an implementation
of a full PSD writer.

## Production safety gates still blocked

1. Define a **background-layer exclusion** for any future rename candidate
   unless Photoshop validates a separate safe semantic treatment.
2. Establish robust Unicode / Pascal legacy-name consistency for ordinary
   layers, including non-ASCII names and editor localization behavior.
3. Obtain real Photoshop **save and reopen** and compositing/metadata
   fidelity evidence, rather than only read-only layer-name inspection.
4. Validate malformed and future arbitrary PSD features with a strict,
   raw-block-based deny-by-default input scanner.
5. Before any productization, implement version-bound isolated output,
   verified stage, no-clobber, full post-write receipt, Windows/Linux
   platform replay and an explicit product release decision.
6. Keep unknown name length, opaque content and background semantics
   blocked; never broaden M4c-2's four-hash research allow-list
   automatically.

This research does **not** add a public `yu psd rename` / `yu psd save`
command or change the frozen v0.1 technical release candidate.

## Evidence

- [psd-tools read-only reference](../../crates/yu-psd-spike/adapters/m4c6_psd_tools_reader.py)
- [Cross-parser and length plans](../data/m4c6-dual-name-fidelity-v1.json)
- [Real Photoshop read-only receipt](../data/m4c6-photoshop-name-receipt-v1.json)
- [M4c-6 validation record](../testing/m4c6-dual-name-photoshop.md)
