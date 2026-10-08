# ADR 0014: M4c-2 PSD Mutation Input Scope and Contract Freeze

- **Status:** Accepted — research/preflight contract only
- **Date:** 2026-10-09
- **Milestone:** M4c-2
- **Base:** `develop@ff61d9aa975e9decc5f19bd555e0d916e529060d`
- **Frozen v0.1:** `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af` remains unchanged

## Decision in brief

**Freeze a deliberately narrow, fail-closed PSD mutation research contract.
Do not authorize general PSD writes or expose a new production CLI.**

The M4c-1 experiment demonstrated that `ag-psd` can modify and round-trip
selected layer metadata, but **even a no-op save** can change effects,
image-resource data or composite pixels. A parsed, 8-bit RGB PSD is not
therefore automatically safe to rewrite.

The only M4c-2 admission state is `research_candidate`. Admission is
**content-hash-bound to four committed, already studied fixture identities**,
not a general file-type, directory, customer-file or version-pattern
allow-list. This prevents an implementation from interpreting normalized
PSD metadata as proof that unknown opaque Photoshop blocks were preserved.
Copied files with precisely identical bytes qualify as the same known
experimental fixture; even a one-byte change invalidates that identity.

## Frozen gates

| Gate | Required behavior |
|---|---|
| Engine | ag-psd exactly 31.0.2, Node exactly 22.23.3, no fallback |
| Input | Regular file only, PSD/PSB header, RGB/8-bit, within 8 MiB, 2 million pixels, 64 logical layers and 32 nesting levels |
| Unknown metadata | **Default deny.** Only the four exact SHA-256 fixture identities pass the experimental gate |
| Mutation | `rename` is the only research candidate; show/hide, opacity and all other modifications are blocked |
| Layer | Canonical preorder ID plus the expected old name; never resolve by name alone |
| Approved target | Exact reviewed ID and kind (pixel/group) from the known fixture profile, no selected masks/effects |
| New name | NFC Unicode, 1–80 code points, no control characters or leading/trailing whitespace; not the old name |
| Source version | Explicit `expected_source_sha256`, checked against actual input bytes |
| Output | Explicit new PSD/PSB path, correct extension, existing directory, distinct from input and initially absent (including symlinks) |
| Preflight | Read-only plan. No staging file, no output file, no sidecar lock, no mutation approval token |
| Production write | **Forbidden by this ADR**, irrespective of a green preflight plan |

The research-only machine policy is
[`psd-safe-mutation-scope-m4c2-v1.json`](../data/psd-safe-mutation-scope-m4c2-v1.json).
The exact filename/hashes and approved target IDs are stored there and
cross-checked against [M4c-1 evidence](../data/m4c1-psd-mutation-evidence-v1.json).

### Separate admission stages for future production work

Do **not** silently expand the four-hash research profile into a broad
user-file allow-list. General admission requires a separate audited
Photoshop PSD/PSB raw-block inventory that detects unsupported
additional-info blocks, color/ICC profiles, adjustment content,
linked/embedded assets, unknown blend/effect metadata and unresolved
inter-application compatibility. The parser must reject unknown fields
instead of relying on ag-psd normalization, which may omit them.

Photoshop (or equivalent independent full-fidelity editor) open/save/reopen
and appearance/metadata checks are also required before a public write
gate can be considered. In particular, stale composite thumbnails after
visibility/opacity changes are not fixed by a successful metadata parse.

## Request/response contract

The experimental preflight consumes exactly one JSON request from stdin:

```json
{
  "contract_version": "1",
  "operation": "rename",
  "input_path": "/absolute/or/relative/source.psd",
  "expected_source_sha256": "<existing-source-sha256>",
  "layer_id": "L0001",
  "expected_old_name": "Old name",
  "new_name": "New name",
  "output_path": "/absolute/or/relative/new-output.psd"
}
```

An accepted preflight returns an envelope with `status: "ok"` and a
`plan` containing:

- `contract_version: "1"`, `operation: "psd.layer.rename"`;
- `status: "research_candidate"`, `write_authorized: false`,
  `dry_run: true`, `side_effects: "none"`;
- source path, byte SHA-256, fixture profile, format/header, layer count;
- selected canonical layer ID/kind and previous/new names;
- absent new output path with `policy: "new_file_only"`;
- pinned engine versions, policy content hash and outstanding safety gates.

The plan never returns output bytes, a completed mutation hash, a
permission token, a file replacement authorization or a claim that the
future writer will succeed. It is advisory and must be **revalidated**
against the current source and filesystem before any future write.

Rejected requests return `status: "error"`, a stable
`error: { code, message }`, and the following exit classes:

| Code | Exit | Meaning |
|---|---:|---|
| `INVALID_ARGUMENT` | 2 | Bad/extra/missing fields, layer-ID syntax, unsupported name or output suffix |
| `INVALID_INPUT` | 2 | Missing input or output directory |
| `SOURCE_VERSION_CONFLICT` | 2 | Source bytes or requested previous layer name differ |
| `OUTPUT_CONFLICT` | 2 | Output already exists or aliases source |
| `UNSUPPORTED_DOCUMENT` | 3 | Unknown fixture bytes, disallowed layer/profile or PSD features |
| `UNSUPPORTED_OPERATION` | 3 | Non-rename mutation |
| `ENGINE_INCOMPATIBLE` | 3 | Wrong Node/ag-psd versions or engine missing |
| `EXECUTION_FAILED` | 1 | Internal preflight execution problem |

This is not an extension to PSD Capability Contract v1 or Managed ag-psd
Protocol v1. The standalone `yu-psd-spike` script remains research-only
and is deliberately not included in runtime capability discovery.

## Future write contract (design constraint only, not implemented)

If an independent release decision later authorizes restricted rename:

1. Reopen the exact source, recheck its byte SHA-256 and target old name/ID
   immediately before a write; prohibit source-file writes by default.
2. Require an explicit distinct new output path. Do not permit overwrite
   without a later, separately accepted PSD replacement contract.
3. Keep the Managed engine isolated; use a staging file and strict expected
   output structure. Verify that **only the requested name changed**, while
   all required layer/resource/image fingerprints remain identical.
4. Reopen the published output and produce a typed receipt with old source
   digest, new output digest, format, bytes and verified invariants, following
   M4b-3 semantics. Report post-publication validation failure explicitly;
   never claim atomic compare-and-swap with uncooperative external writers.
5. Treat any unknown opaque data, composite mismatch, resource change, or
   inability to independently prove Photoshop fidelity as a refusal.

**No production writer exists as a consequence of this freeze.**

## Alternatives rejected

- **Admit any 8-bit RGB file:** rejected; cannot detect unparsed features by
  looking only at the normalized ag-psd object.
- **Enable opacity/visibility when property rereads correctly:** rejected;
  cached composite fidelity is unresolved.
- **Allow writes with a strong warning:** rejected; a warning cannot restore
  silently removed PSD resources or layers.
- **Modify original PSD in place:** rejected; M4b's source safety and
  independent validation requirements still apply.
- **Auto-fallback to another engine:** rejected; cross-engine fidelity and
  high-bit semantics are not equivalent.

## Evidence and acceptance

See [M4c-2 acceptance](../testing/m4c2-psd-input-scope-freeze.md),
[M4c-1 investigation](../psd-mutation-feasibility-m4c1.md) and
[the pure-read-only preflight](../../crates/yu-psd-spike/adapters/typescript/m4c2_safe_mutation_preflight.cjs).
Until Linux/Windows replay, unknown-block audit and independent editor
fidelity tests are complete, **public PSD mutation remains BLOCKED**.
