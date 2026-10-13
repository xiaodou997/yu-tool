# ADR 0027 — M4c-11 PSD Metadata Semantics and Research Output Publication

**Date:** 2026-10-13

**Decision:** accept research-only output safety prototype and scoped metadata classification.
**Public PSD mutation remains BLOCKED.**

**Base:** `develop@5b20e4c3c196ac5933305bf6257e2c00df5ab137`.
The v0.1 `main` freeze at
`5526dc4465c4e79ad7f2757d06b3b1693642b8af`
is not modified.

## 1. Why another gate is needed

M4c-10 showed successful Photoshop 2026 save/reopen of two
SHA-pinned multilingual synchronized names and preservation of
`lyid` under two native structure operations. These results do not
establish that arbitrary XMP, EXIF, layer `shmd`, or undocumented
PSD resources may be discarded or modified.

Publication safety is an **independent** concern. Even a correct byte
patch must never overwrite a user file or publish a stale source
version. This ADR deliberately keeps publication and Photoshop
metadata fidelity as separate gates.

## 2. Photoshop-saved metadata: more precise, not less strict

An independent, strictly read-only decoder audited **two**
Photoshop-native M4c-10 saved control/candidate pairs. The original
saved files were located by **committed exact SHA-256 receipts**.

| Field | What this experiment establishes | What remains blocked |
|---|---|---|
| EXIF Resource 1058 | TIFF parses on both sides: **3 IFDs and 22 tags**, with no tag set additions/removals; changing field is tag **306 / DateTime** in both pairs | Other EXIF revisions, private tags and semantic fidelity cannot be declared safe |
| XMP Resource 1060 | Changed `DocumentID`, `InstanceID`, `History`, `ModifyDate`, `MetadataDate` are separately fingerprinted | Document/provenance identity and editing history must **not** be stripped or globally ignored |
| Slices Resource 1050 | Each observed Slices resource differs only in its serialized save-document name; all other Slices bytes agree | Filename-related changes are not a global rewrite permission |
| Layer `shmd/cust` | In both multilingual saved pairs, raw custom metadata entry bytes are identical between unchanged control and edited candidate | Unknown future variants of `cust` remain denied; prior M4c-9 `layerTime` structure recognition is fixture-scoped |
| Raw channels / compressed composite | Both saved pairs have identical channel/composite payloads | This does not replace native Photoshop rendering/metadata acceptance on arbitrary inputs |

The new bounded TIFF decoder checks endian marker, TIFF magic, IFD
count, recursion, entry types and data bounds, including sub-IFDs.
Unrecognized data fails closed. It returns hashes and field identities,
not a blanket normalized metadata representation.

**The metadata exclusion policy is empty.** A tag being called
`DateTime`, or matching a time-like number, is not enough to ignore
its loss. See
[M4c-11 metadata evidence](../data/m4c11-metadata-semantic-gate-v1.json).

## 3. Private research-only output transaction

The standalone
[`m4c11_safe_output_spike.cjs`](../../crates/yu-psd-spike/adapters/typescript/m4c11_safe_output_spike.cjs)
models the existing PSD PNG export pattern without exposing a PSD
writer to the public CLI.

It only accepts:

- the **exact** committed M4c-8 8-bit RGB pixel fixture with the
  pinned source SHA-256, or a byte-identical **regular** research
  scratch copy;
- the deterministic **`Layer 1 → Layer A`** name-byte candidate,
  validated against its separately frozen SHA-256;
- one fixed output name inside a new ignored
  `target/m4c11-safe-output/run-*` directory.

The experiment:

1. Checks the source is an ordinary non-symlink file of permitted size
   and matches its expected SHA-256.
2. Generates a fixed-width, dual-name patch with exactly the expected
   bytes; checks the output's raw structure, names and SHA-256.
3. Creates a private same-filesystem staging directory and writes
   the candidate with exclusive creation and `fsync` on the file.
4. Rechecks source device/inode, length and SHA-256 before publication.
5. Calls the filesystem's atomic **hard-link** operation from staging
   to an absent destination; any existing file, symlink or racing
   publisher wins and is never overwritten.
6. Reopens and rehashes the published output; retains a receipt and
   cleans private staging.

**Seven negative/positive targeted tests** cover deterministic source,
already-existing output, symlink output, a competing publisher, source
byte changes during staging, source replacement with same content,
source symlink, wrong SHA and prohibited paths. Tests never publish
outside ignored research scratch space.

[Output test evidence](../data/m4c11-safe-output-v1.json).

### The remaining transaction caveats

The hard-link no-clobber property is useful and already used in the
read-only PNG exporter, but this experimental implementation is **not
a production transaction**:

- A source can theoretically be changed after the final source
  recheck; there is no filesystem transaction that atomically binds
  its content to the new output.
- The parent directory is not durably fsynced. Filesystems without
  same-volume hard-link support are rejected; portability and power
  failure recovery have not been proven.
- If validation after publication fails, the output might already
  exist. This is **inconclusive**, not a successful output receipt;
  never blindly delete a competing file or repeat the save.
- Only one exact SHA-256 candidate is accepted. There is no general
  PSD input, variable-length name support, arbitrary `lyid`
  selection, Photoshop write/sync, app-facing command, or production
  capability declaration.

## 4. Exit criteria and recommendation

- [x] Scoped EXIF TIFF tag-level classification with strict decoding.
- [x] XMP identity/history changes surfaced; exclusion policy remains
  **none**.
- [x] Existing saved-pair Slices and `shmd` comparisons retained.
- [x] SHA-pinned research output staging, hard-link no-clobber,
  output validation and negative race/version tests.
- [ ] Full semantic proof for arbitrary XMP, EXIF, `shmd` and unknown
  Photoshop features.
- [ ] Crash-durable output commit/receipt, filesystem portability,
  native Photoshop format fidelity and full customer source-version
  concurrency guarantees.
- [ ] Separate release review for a public `yu psd rename` feature.

**M4c-11 research exit:** publish the tested evidence to `develop`
without changing `main`, the production capabilities list or the
`PSD_PRODUCTION_MUTATION_BLOCKED` policy.
See [M4c-11 acceptance](../testing/m4c11-metadata-publication-acceptance.md).
