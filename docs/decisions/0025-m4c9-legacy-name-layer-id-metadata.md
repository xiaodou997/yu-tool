# ADR 0025 — M4c-9 Legacy PSD Name Encoding, Layer IDs and Metadata Semantics

- **Date:** 2026-10-13
- **Status:** Bounded evidence accepted, general mutation **BLOCKED**
- **Base:** `develop@24440751bf059d8696ce639e6d603ddc611d028b`
- **Protected `main`:** `5526dc4465c4e79ad7f2757d06b3b1693642b8af`

## Decision

The M4c-9 milestone resolves two compatibility questions *for fixed,
SHA-256-bound Photoshop-generated research fixtures*: how Photoshop
2026 encodes two non-ASCII legacy Pascal layer names in this local
environment, and whether a duplicate-name pixel layer can be
disambiguated via its native PSD `lyid` and Photoshop DOM `ArtLayer.id`.

Neither result authorizes arbitrary Unicode rename, in-place PSD write
or a public `yu psd rename` subcommand.

## 1. Pascal and Unicode names: not necessarily divergent

The M4c-8 Adobe Photoshop 2026 *native-saved* scratch PSDs were
re-checked against exact SHA-256 hashes from their committed v1
receipt. The raw scanner treats the Pascal field as Latin-1 only
to preserve arbitrary bytes for display; the resulting apparent
mojibake is not a character-encoding interpretation.

| Photoshop layer | Raw legacy Pascal bytes | GB18030 decoded value | `luni` |
|---|---|---|---|
| `测试图层` | `b2e2cad4cdbcb2e3` | `测试图层` | `测试图层` |
| `Слой` | `a7b3a7dda7e0a7db` | `Слой` | `Слой` |

**Both fields represent the same names in these particular original
Photoshop files when decoded as GB18030.** This is a sample-scoped,
environment-dependent finding, not proof that Photoshop universally
encodes Pascal layer strings using GB18030, regardless of operating
system, language, locale or historical PSD producer. Do not replace
the inventory's lossless raw byte representation or silently assume
a locale-dependent codec. Unknown encoding must fail closed.

M4c-8's `luni`-only edits were still semantically incomplete:
they changed the Unicode name but **left original legacy bytes**, which
would decode to the *old* names in these two fixtures. That strategy
remains denied for publication.

## 2. Stable identification of duplicated display names

The same Photoshop-native specimen contains two pixel layers both
named `Layer 0`; one has PSD Additional Layer Information
`lyid=6`, the other `lyid=2`. Both IDs are distinct and are
confirmed through Photoshop's native `ArtLayer.id` read-only API.

The research-only, fixed-width dual-name patch selected **`lyid=6`**,
not the text `Layer 0`, changed its Unicode and seven-byte ASCII
Pascal representations to **`Layer A`**, and left `lyid=2`
unchanged. Exactly **2 source bytes** changed inside explicitly
allowed name ranges, with unrelated raw Image Resources, channel data
and merged composite protected. Photoshop saved the scratch candidate,
closed and reopened it; both IDs retained the expected names:

| Native PSD layer ID | Before | After re-open |
|---:|---|---|
| 6 | `Layer 0` | `Layer A` |
| 2 | `Layer 0` | `Layer 0` |

An ID can only be used after verifying it is **unique in the exact
SHA-bound document** and bound to the expected raw layer record and
logical target. Neither `lyid` nor a name alone establishes a globally
stable identity across unrelated PSD files or transformations.

This is *experimental* Photoshop reserialization. Photoshop may change
opaque document metadata even when names and compressed image data
are preserved; no product write permission follows from native
name persistence.

## 3. `shmd/cust`, XMP, and EXIF

Previously the whole 52-byte `shmd/cust` metadata item was opaque.
The bounded research parser now recognizes a Photoshop Descriptor
structure with a `metadata` class and an eight-byte
`layerTime` **double** field at offset 43, followed by a zero byte,
on every one of four layers in each of two control/candidate save
pairs. The fixed 43-byte prefix agrees between each pair and the
numeric values vary.

This is **evidence of a layer-time-like value**, not proof that all
`cust` entries or all variations are safe to discard. Other
descriptor shapes must fail closed. The user should not assume this
is the only information carried by metadata or that modification
times can always be regenerated without loss.

- Image Resource **1060**: XMP document IDs, instance IDs,
  modification timestamps and history are observed to differ in
  native Photoshop saves. These are provenance/identity fields,
  not universally ignorable metadata.
- Image Resource **1058**: EXIF TIFF header `4d4d002a` is verified;
  some native comparisons differ in serialized bytes. Full EXIF
  semantics are **not independently certified**.
- `shmd/cust`: `layerTime` field recognized on **16 metadata
  instances** (four layers × control/candidate × two runs); byte
  preservation and complete semantic safety remain unproven.
- Image Resource **1050**: prior M4c-7d demonstrated the 16-byte
  scratch-file-name difference in Slices data for a single fixture;
  it is **not** a general omission/normalization rule.

See [machine-readable encoding/identity experiment](../data/m4c9-encoding-lyid-v1.json)
and [metadata evidence](../data/m4c9-metadata-semantics-v1.json).
Adobe's public PSD format specification provides the normative
`lyid`, `luni`, Image Resource and metadata-block layouts:
https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/

## 4. Research exit conditions / next work

- [x] Legacy byte mapping *for the two frozen native files*
  reconstructed without changing the raw name inventory.
- [x] Duplicate display names resolved via independently observed
  `lyid`, with a native Photoshop scratch save/reopen experiment.
- [x] Partial `shmd/cust` descriptor shape and XMP/EXIF resource
  classification demonstrated.
- [ ] Multiplatform/multilocale Pascal encoding matrix, codec
  provenance, non-ASCII **dual-name synchronized** patch and
  Photoshop save/reopen on new exact-hash fixtures.
- [ ] Strong identity preservation under reorder/group conversion
  and other native PSD editing operations.
- [ ] Detailed semantics and retention of XMP, EXIF and arbitrary
  `shmd/cust` Photoshop features.
- [ ] Independent full-fidelity production publication and M4 release
  decision.

**Final M4c-9 decision: PSD production mutation remains BLOCKED.**
The frozen v0.1 `main` branch and the existing public read-only
PSD capabilities remain unaffected.

See [acceptance](../testing/m4c9-encoding-lyid-metadata-acceptance.md).
