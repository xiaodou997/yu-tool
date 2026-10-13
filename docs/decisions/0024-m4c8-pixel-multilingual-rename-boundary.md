# ADR 0024 — M4c-8 Ordinary Pixel and Multilingual PSD Rename Evidence

- **Status:** Research experiments complete; production PSD mutation **BLOCKED**
- **Date:** 2026-10-13
- **Base:** `develop@30db4036368e3961213f4e69555dda9d51f00d35`
- **Frozen v0.1 `main`:** `5526dc4465c4e79ad7f2757d06b3b1693642b8af`

## Decision

Broaden **research evidence**, not production writing. M4c-8 validates
one exact-SHA ordinary Photoshop pixel-layer fixed-width rename and
investigates multilingual and duplicate-name behavior on Photoshop-native
scratch copies. All public writer capabilities remain **absent**.

See the [M4c-8 research-only policy](../data/m4c8-limited-rename-policy-v1.json).

## Ordinary RGB pixel layer result

The previous M4c-7e read-only review identified
`bench-baseline-rgb` (`background-red-opacity-80.psd`) as a
promising specimen with two native **ordinary** pixel layers, no native
Background, and matching original ASCII Pascal and Unicode names.

M4c-8 pins its source SHA-256 to
`391f4c009b9bd4cc5fdffb3799c3476f7d690874e6ca458463ab7fe3b5739aee`
and changes only raw layer index 0's `Layer 1 → Layer A` (seven UTF-16
code units in both names). The standalone research implementation:

1. Requires exactly the frozen source bytes, PSD 8-bit RGB, exactly
   two raw layers, four channels on the target and the expected names;
2. Alters only the target `luni` UTF-16BE bytes and corresponding
   seven-byte ASCII Pascal name, never another document byte;
3. Confirms raw Image Resources, layer-channel descriptors, merged
   composite, header and source SHA-256 remain unchanged;
4. Produces **distinct disposable input copies** and sends them to
   Adobe Photoshop 2026 only if the app responds and has zero open docs;
5. Uses Photoshop's native saveAs into two new scratch PSD paths, then
   closes and reopens each for native-name and independent psd-tools
   logical-tree/pixel checks.

The two name encodings required **2 changed bytes** total. Photoshop
opened, saved and reopened both files; `Layer A` persisted, unrelated
native names were unchanged, and the independent decoded saved control
and edited candidate composites were equal. Raw channel and compressed
merged-composite bytes on the saved pair were identical.

**But the strict saved-pair metadata gate fails.** Resource `1060`
(XMP), `1050` (Slices) and `1058` (EXIF) differ. Saved control and
candidate sizes also differ, and their full opaque resource semantics
were not independently certified. A successful native name save is
not permission for arbitrary PSD editing.

[Pixel research receipt](../data/m4c8-pixel-photoshop-fidelity-v1.json).

## Photoshop-native multilingual and duplicate-name behavior

Separately, Photoshop itself renamed the ordinary pixel layer on
**fresh disposable copies** of this same exact source into:

| Case | Photoshop native name after save/reopen | Independent psd-tools name | Legacy raw bytes |
|---|---|---|---|
| Chinese `测试图层` | Matched | Matched | Not identical to Unicode when interpreted as Latin-1 |
| Cyrillic `Слой` | Matched | Matched | Not identical to Unicode when interpreted as Latin-1 |
| Duplicate `Layer 0` | Two layers share the name | Both `Layer 0` | ASCII representations match |

For non-ASCII names, Photoshop stores a distinct byte encoding in the
legacy Pascal field. The M4c research scanner deliberately represents
these raw bytes using Latin-1 for lossless inventory; **this is not
proof that Photoshop's legacy field is corrupt or semantically
different**. The actual legacy character mapping and safe
round-trip treatment require independent research.

On two additional **scratch-only** byte-patch experiments, changing
just the Chinese Unicode `luni` name
`测试图层 → 实验图层` and the Cyrillic
`Слой → Тест` let Photoshop read the new Unicode names.
However, the original legacy Pascal **bytes remained unchanged**.
This *demonstrates* a potential dual-name inconsistency and cannot be
used as an allowed production patch merely because the current
Photoshop version displays the `luni` name.

For duplicate names, Photoshop permits equal human-readable names;
selecting a target by `name` is ambiguous. A stable canonical ID
and independently verified raw record identity must be required.

[Native names receipt](../data/m4c8-multilingual-photoshop-names-v1.json),
[Unicode-only refusal study](../data/m4c8-luni-only-multilingual-v1.json).

## Safety boundary

- All sources have pinned exact SHA-256 provenance; scratch outputs are
  created new under ignored `target/` and never replace input files.
- The normal pixel experiment adds **no entry** to frozen M4c-2/M4c-7a
  admitted production writers, because none exist.
- Background, Type, Shape, effects, masks, Smart Objects,
  non-RGB/high-bit, different-size UTF-16BE edits, unversioned
  sources and name-only duplicate selection remain blocked.
- EXIF, XMP document provenance, Slices and `shmd/cust` metadata
  must not be globally ignored or silently rewritten for a green test.
- No Photoshop native save result, even successful, implies broad
  native editor rendering fidelity on arbitrary customer PSD/PSB.

## Next research gate

M4c-9 can test Unicode/legacy charset mappings on more controlled
Photoshop versions, introduce exact-hash duplicate-name ID-binding
cases, and audit XMP/EXIF/Slices/`shmd` provenance semantics. Only
after independent evidence and a separate version-bound output
publication decision should an **opt-in**, new-output-only PSD
rename capability be proposed. **Production write remains BLOCKED.**

See [M4c-8 acceptance](../testing/m4c8-pixel-multilingual-acceptance.md).
