# ADR 0017 — M4c-5 PSD Fixed-width Layer Name Byte Patch Feasibility

- **Status:** Research accepted; public PSD mutation remains **BLOCKED**
- **Date:** 2026-10-10
- **Baseline:** `develop@3c8769c93389f9d24847df2c3e9748852d57eb7d`
- **v0.1 freeze:** `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af` unchanged

## Decision

**Continue experimenting with a narrowly targeted byte-preserving layer
rename, instead of saving all Photoshop document content through a general
PSD writer. Do not expose this as a production feature.**

M4c-4 showed that an ag-psd no-op rewrite modifies raw data for all 11
successfully saved files and removes opaque resources on multiple samples,
with a decoded composite pixel mismatch on one. M4c-5 tests whether
altering only the existing, length-preserving Unicode `luni` layer name
inside an otherwise untouched copy can avoid those unintended changes.

## Frozen experiment and guardrails

The isolated
[`m4c5_byte_patch_spike.cjs`](../../crates/yu-psd-spike/adapters/typescript/m4c5_byte_patch_spike.cjs)
is **not** in `yu` capability discovery, a Managed engine entrypoint, or
the public PSD protocol. It takes no user-supplied document paths, requests
or destination arguments. It selects four exact-hash research fixtures
from the M4c-2 policy and produces disposable candidate files exclusively
under ignored `target/m4c5-psd-byte-patch/run-*`.

For each case it:

1. Calls M4c-2's **read-only preflight** to confirm source SHA-256,
   expected old name, canonical logical layer ID, pinned engine and absent
   separate target. Preflight still says `write_authorized=false`.
2. Uses the independent M4c-3 scanner to locate precisely one `8BIM/luni`
   block for an explicitly researched **raw layer record index**. The
   canonical logical layer ID is separately checked using ag-psd; a Photoshop
   section-divider sentinel is **not** a logical user-facing layer.
3. Verifies the original `luni` string and its declared UTF-16BE code-unit
   count. Only a replacement with **exactly the same UTF-16 code-unit
   length** is supported. No section lengths, record counts, offsets or
   padding bytes may change.
4. Makes a buffer copy and updates the target `luni` payload. Where the
   legacy Pascal name is matching ASCII and has exactly the same byte length,
   the experiment also updates that independent name representation.
   It never blindly replaces matching text elsewhere in the file.
5. Checks **every byte** outside the explicitly allowed name ranges is
   unchanged. It reparses with the raw scanner and rejects changed image
   resources, layer channel bytes, merged image bytes, document/header fields
   and any non-target tagged block.
6. Reopens both documents via pinned **Node.js 22.23.3 /
   ag-psd 31.0.2** read-only API and checks that only the selected logical
   layer name changed, including the nested-group and duplicate-name cases.
7. Writes an experimental output using `wx` (no overwrite) in a new
   ignored run directory; reopens exact candidate bytes and hashes them;
   rehashes the original fixture to confirm it is unchanged.

The evidence is committed in
[`m4c5-psd-byte-patch-evidence-v1.json`](../data/m4c5-psd-byte-patch-evidence-v1.json).

## Reproducible results

| Fixture | Change | Raw record | Canonical ID | Other original bytes | Legacy name handling |
|---|---|---:|---|---|---|
| Two-layer PSD | `Фон` → `Дом` | 0 | `L0001` | unchanged | Original non-ASCII Pascal bytes retained; potential inconsistency |
| Two-layer PSB | `Фон` → `Дом` | 0 | `L0001` | unchanged | Original non-ASCII Pascal bytes retained; potential inconsistency |
| Nested group | `Group 1` → `Group 2` | 3 | `L0002` | unchanged | Both matching ASCII Pascal and Unicode names updated |
| Duplicate names | `X` → `Y` | 1 | `L0002` | unchanged | Fixture's pre-existing divergent legacy name retained |

**4/4** fixed-width experimental copies were patched, reparsed and
byte-difference verified. No file lengths changed, no resources or channel
bytes were re-encoded, no source fixtures changed, and **0 production
writes were authorized**.

This is a more promising **byte-integrity** result than general ag-psd
serialization, but it does not certify Photoshop interoperability, correct
legacy-name display in older editors, or arbitrary PSD edit safety.

## Reasons production mutation stays blocked

**Dual layer-name representations:** Photoshop files can contain both the
Unicode `luni` name and a legacy Pascal name. For 3/4 research cases,
the Pascal representation is not synchronized with the new name; in the
duplicate-name fixture those representations were already divergent
before the experiment. A parser may select one representation and show an
outdated name. There has been **no real Photoshop open/save/reopen test**.

**Length constraints:** The experiment only replaces UTF-16BE code units
of equal length. General renames with different lengths require adjusting
layer extra-lengths, tagged block lengths and enclosing PSD/PSB section
lengths; that is a distinct, higher-risk parser/write design.

**Format and metadata risks:** Only four exact M4c-2 known fixture hashes
are admitted; unknown customer documents, text/shape/effects/Smart Objects,
arbitrary group hierarchies, 16-/32-bit files, malformed/opaque blocks and
visibility/opacity updates remain unsupported. The prototype does not
validate semantic integrity of every opaque Photoshop metadata item.

**Publication:** The experiment writes only `target/` disposable copies,
not user's documents. It does not implement crash durability, Windows
sharing diagnostics, atomic source binding under external writers, generic
PSD overwrite or M4b post-write approval semantics.

## Next recommended gate

M4c-6 should focus on independent editor validation and **dual-name
representation correctness**, especially nested groups, duplicate names,
non-ASCII Unicode, legacy display fallbacks and PSD/PSB interoperability.
Then separately consider a fixed-width, opt-in, output-only rename proposal
with explicit source SHA-256 binding, strict feature admission, staged
verification, and a user-visible failure/receipt contract. Do **not**
open general `yu psd save`, opacity or visibility editing based on this
research.

## Evidence

- [Experimental patch implementation](../../crates/yu-psd-spike/adapters/typescript/m4c5_byte_patch_spike.cjs)
- [Safety and negative tests](../../crates/yu-psd-spike/adapters/typescript/m4c5_byte_patch_spike.test.cjs)
- [Machine-readable byte-level evidence](../data/m4c5-psd-byte-patch-evidence-v1.json)
- [M4c-5 acceptance](../testing/m4c5-psd-byte-preserving-rename.md)
