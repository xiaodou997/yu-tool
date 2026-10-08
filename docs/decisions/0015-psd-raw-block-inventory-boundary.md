# ADR 0015: M4c-3 PSD/PSB Raw Block Inventory and Unknown Metadata Boundary

- **Status:** Accepted — read-only research inventory only
- **Date:** 2026-10-09
- **Milestone:** M4c-3
- **Base:** `develop@00d0c2fb2b67f3fc5561374498d2fb3365358f92`
- **Frozen v0.1:** `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af`

## Decision

Add a **byte-level PSD/PSB structural scanner** with bounded reads, explicit
offsets, exact lengths, fingerprinted opaque payloads and an inventory of
unrecognized Image Resource IDs and Additional Layer Information keys.
The scanner is standalone **research-only** inside `yu-psd-spike`; do not
expose a public `yu psd` mutation operation, change the Managed engine
protocol or broaden the four-hash M4c-2 research admission list.

**Every scan reports `mutation_authorized=false` and
`safe_to_rewrite=false`, even when it sees zero unknown blocks.**
Recognizing an Adobe block signature or a historically documented resource
ID **does not establish** that ag-psd (or YuTool) can faithfully rewrite it.
Raw-block inspection is evidence, not preservation proof.

## Format framing

The implementation follows Adobe's
[Photoshop File Formats Specification](https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/),
especially the five top-level PSD sections, Image Resources, Layer Records,
Additional Layer Information and PSB-specific 64-bit section/channel lengths.

It inventories:

| Part | Check |
|---|---|
| File header | `8BPS`, version 1/2, reserved fields, channel count, dimensions, bit depth, color mode |
| Color-mode data | u32 length, bounded opaque payload |
| Image Resources | `8BIM` signature, 16-bit resource ID, even-aligned Pascal name, u32 payload length, even padding, content digest |
| Layer & mask section | PSD u32 or PSB u64 length, bounded section |
| Layer records | Rect, channel IDs and version-specific lengths, `8BIM` blend signature, blend key, opacity/flags, bounded extra data |
| Layer extra data | Mask length, blending-range length, four-byte-aligned Pascal layer name, tagged Additional Layer Information |
| Tagged blocks | `8BIM` or `8B64`, four-byte key; PSB's specified long keys use u64 lengths; even alignment and payload SHA-256 |
| Layer channel payloads | Respect recorded lengths and section bounds without pixel decompression |
| Global mask / document tags | Length framing, recognized tagged blocks and unknown key inventory |
| Merged image data | Compression marker and remaining opaque byte span; **not** decoded or integrity-verified |

The research scanner has explicit limits: **8 MiB input, 2 million canvas
pixels, 64 directly framed layer records, 128 channels per layer, and 1,024
framed metadata blocks**. Integer overflow, unexpected signatures, invalid
reserved header bytes, truncated length-delimited sections and overlarge
PSB u64 lengths cause structured errors.

Some committed files contain extra zero alignment bytes or omit the otherwise
expected global mask length. The scanner records these deviations as risks,
not as evidence of a standards-compliant, lossless round trip.

## Recognized, unknown, opaque: three different concepts

- **Recognized** means only that an ID/key appears in the research scanner's
  list of known Photoshop field names; **not** that the library understands
  the field's nested structure or can preserve it.
- **Unknown** means the encountered resource ID or four-character key was
  not recognized. Record its exact location, scope, length and fingerprint;
  never silently drop or reinterpret its bytes.
- **Opaque** means the payload was deliberately not decoded/audited for
  semantic equivalence. **All** such data remain a write gate, including
  known IDs, color-mode data, layer-channel data, thumbnails, embedded blocks
  and the compressed merged image.

For PSB, tagged `Layr`/`Lr16`/`Lr32` sections can contain nested layer
records. M4c-3 identifies and fingerprints those tagged sections but does
**not** recursively decode their inner layer tree; the
`top_level_layer_records` count is therefore **not** necessarily the
logical Photoshop layer count in high-bit documents.

Similarly, corruption confined to the tail of a compressed merged image
may not be detectable from the section framing alone. A valid inventory
**does not prove image decode success, Photoshop visual fidelity, unknown
additional-info preservation or safe document saving**.

## Evidence

- [Implementation](../../crates/yu-psd-spike/adapters/typescript/m4c3_raw_block_inventory.cjs)
- [Adversarial tests](../../crates/yu-psd-spike/adapters/typescript/m4c3_raw_block_inventory.test.cjs)
- [Deterministic corpus scanner](../../crates/yu-psd-spike/adapters/typescript/m4c3_raw_inventory_corpus.cjs)
- [Machine-readable inventory](../data/psd-raw-block-inventory-m4c3-v1.json)
- [Local acceptance](../testing/m4c3-psd-raw-block-inventory.md)

The committed corpus has **13 valid PSD/PSB fixtures**, all read-only scanned,
and **1 deliberately malformed input**, rejected. **11 of 13 valid fixtures**
have at least one unknown metadata block under this research vocabulary.
The two files without unknown keys are still explicitly **not safe to
rewrite** because recognized opaque blocks and merged image semantics have
not been independently audited.

## Unchanged production boundary

ADR 0014's four exact approved *research fixture* identities remain the
only M4c-2 preflight candidates. M4c-3 may inspect **other** files
read-only to identify risks, but inspection **does not admit them** into any
PSD mutation or publishing contract.

No new `yu capabilities` entry, real rename, layer visibility/opacity
mutation, PSD/PSB output writing or original-file replacement is implemented.
M4b safe output publication applies to raster operations only until a
separate, validated PSD mutation integration exists.

## Follow-up / non-negotiable gates

1. M4c-4: expand recursive parsing of nested `Layr`/`Lr16`/`Lr32`,
   inspect known blocks' nested substructures and distinguish truly
   preservation-audited fields from merely recognized field IDs.
2. Differential, **no-op** round-trip comparison of **every** opaque
   block using ag-psd and an independent parser/editor. Any dropped/changed
   unknown block or composite fingerprint must block mutation.
3. Independent Photoshop or equivalent editor open/save/reopen verification,
   including composite appearance, thumbnail consistency, ICC, masks,
   effects, text, Smart Objects and linked assets.
4. Only then propose an updated, versioned, feature-level allow-list and
   guarded output-only rename contract. Do not enable real writes merely
   because an inventory has no unknown keys.
5. Windows/Linux independent replay is pending while GitHub Actions quota
   is unavailable. macOS-only tests are not multi-platform proof.
