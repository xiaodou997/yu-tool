# Agent Guide

YuTool is designed so coding agents can discover and invoke local tools without learning every backend-specific CLI.

The public command is:

\`\`\`bash
yu
\`\`\`

## Accepted implementation baseline

M3 is frozen at `c793279e3c131cae85adf73571590ab022b2ed05`; read [the freeze receipt](milestones/m3-freeze.md) and [machine-readable evidence](data/m3-implementation-freeze-v1.json) before claiming supported PSD behavior. Five operations are wired only through the exact active Managed package. Historical green checks are not proof for later changes, and prototype artifact URLs are not a public catalog. Rendering, high-bit/non-RGB export and mutation remain deferred.

## v0.1 preparation

Read [release readiness](releasing/v0.1-readiness.md) before distributing a build. Explicit offline installation uses `engine install --manifest FILE --archive FILE`, then a separate activation. Use tracked lockfiles and `cargo --locked` subcommands; never delete the application's Cargo.lock to clean a worktree. Candidate metadata intentionally says `public_release_ready: false`. A repeated test or retry success is not proof that the Windows quarantine root cause is fixed. The [PR #25 checklist](testing/pr25-release-readiness.md) distinguishes core/debug tests from extracted release-binary evidence.

## PSD cleanup failures

PR #28 makes process cleanup an explicit part of the result: valid response bytes or an existing private staged PNG do not establish success when cleanup fails. Preserve the returned `EXECUTION_FAILED` and selected engine; do not publish a leftover private artifact or automatically retry against a different engine. PR #31 replaces blocking transport workers with nonblocking endpoints and a shared two-second cleanup observation budget. It does not merge #29 or prove Windows root-cause closure/release readiness. Read [current scope](reliability/bounded-process-io.md) and [acceptance](testing/pr31-bounded-process-io.md); do not treat a cleanup deadline error as evidence all processes exited.

## Combined Windows startup

PR #32 adapts creation-time Job ownership onto the #31 nonblocking transport. Do not merge the old #29 branch on top: its blocking interfaces and failed independent gate remain historical evidence. Use the [joint acceptance](testing/pr32-windows-owned-bounded-runtime.md) to verify startup, I/O cancellation and cleanup on the same candidate. A successful run is not proof of the historical OS5 cause or release approval.

## Recommended workflow

Agents should use this sequence:

\`\`\`text
1. discover
2. inspect
3. plan
4. dry-run when available
5. apply
6. validate
7. report
\`\`\`

## 1. Discover

Before relying on an optional capability:

\`\`\`bash
yu capabilities --json
yu engine list --json
\`\`\`

Use \`yu doctor --json\` when the environment appears unhealthy.

Do not assume ImageMagick, psd-tools, FFmpeg, or another optional engine is installed.

## 2. Inspect

Inspect the input before mutation.

Examples:

\`\`\`bash
yu image info photo.jpg --json
yu psd inspect design.psd --json
yu psd tree design.psd --json
\`\`\`

Prefer machine-readable output over parsing terminal tables.

## 3. Plan

Use capability and document metadata to decide what operation is safe.

For layered formats, use stable IDs/selectors from YuTool output rather than relying only on display names.

Never assume a layer name is unique.

## 4. Dry-run

When a mutating command supports it:

\`\`\`bash
yu ... --dry-run --json
\`\`\`

A dry-run should be preferred when:

- multiple files will change;
- source overwrite was requested;
- an operation changes document structure;
- the selected engine has limited format fidelity.

## 5. Apply

Write to a new output path by default.

Example:

\`\`\`bash
yu image resize input.jpg --width 1024 -o output.jpg --json
\`\`\`

Avoid \`--overwrite\` unless preserving the original is unnecessary and the user intent is explicit.

## 6. Validate

Inspect or render the result after meaningful mutation.

Examples:

\`\`\`bash
yu image info output.jpg --json
yu psd render output.psd -o preview.png --json
\`\`\`

Where multiple engines exist, future verification workflows may compare engines for sensitive file formats.

## 7. Report

Report:

- output path;
- operation performed;
- selected engine when relevant;
- warnings;
- unsupported features encountered.

Do not report success merely because an external process exited; use YuTool's structured result.

## Engine installation

Managed engine installation should be an explicit action.

If a required engine is unavailable, an agent may suggest or request:

\`\`\`bash
yu engine install --manifest ./trusted-engine-manifest.json
yu engine activate <engine-id> <version>
\`\`\`

Agents should not bypass YuTool and silently install system packages unless the user explicitly requests system-level installation.

## Engine selection

Normal calls should allow YuTool to resolve an engine automatically.

Use an explicit engine only when:

- reproducing a result;
- debugging;
- testing compatibility;
- a particular engine is required for fidelity.

Example:

\`\`\`bash
yu image resize input.jpg --width 1024 --engine raster-rs -o output.jpg
\`\`\`

## JSON behavior

In \`--json\` mode:

- parse stdout as the structured result;
- treat stderr as diagnostics;
- use \`schema_version\` when present;
- branch on stable error codes rather than message text.

Do not scrape human tables when JSON output is available.

## Errors

Important initial error categories include:

\`\`\`text
INVALID_ARGUMENT
INVALID_INPUT
UNSUPPORTED_CAPABILITY
ENGINE_UNAVAILABLE
ENGINE_INCOMPATIBLE
EXECUTION_FAILED
OUTPUT_CONFLICT
VERIFICATION_FAILED
\`\`\`

An unavailable capability is different from a corrupt input. Agents should surface that difference to the user.

## PSD guidance

PSD/PSB support can vary significantly by document feature and engine.

PR #22 supports read-only inspect/tree/layer-list/layer-info through the active Managed ag-psd package. Use `--id L0001` rather than a name, and keep the selected engine/version with the result. PR #23 adds `yu psd layer export <file> --id <id> -o <new.png>` for stored 8-bit RGB layer bitmaps through the `.yu2` package. This preserves raw bitmap alpha but does not apply masks, effects, opacity, blending or ICC conversion; report the returned warning. Groups, absent bitmaps and other bit depths/color modes are unsupported. The destination must not exist, and its parent filesystem must support hard links. Rendering and mutation remain deferred. Missing or inactive engines require explicit installation/activation; do not retry using an implicit alternative. `--timeout-secs` defaults to 30 and is bounded to 1..3600.

In JSON mode, success is one stdout envelope; errors, including argument errors, are one stderr envelope. Selected PSD engine metadata may be attached to failures. Do not treat an engine's inventory-wide capability list as the active version's execution contract.

Agents should:

- inspect first;
- preserve the source file;
- report selected engine;
- treat fidelity warnings seriously;
- avoid claiming unsupported text/shape/Smart Object edits succeeded;
- render/validate after structural changes.

## Security guidance

Do not turn YuTool errors into arbitrary shell execution.

If an engine is missing, prefer YuTool's managed-engine path or explicit user-directed installation instructions.

Do not interpolate file names into shell strings. Use structured command invocation when integrating YuTool programmatically.

## Example agent session

\`\`\`bash
# Discover
yu capabilities --json

# Inspect
yu psd tree poster.psd --json

# Inspect one selected layer (read-only)
yu psd layer info poster.psd --id L0012 --json

# Export only when psd.layer.export is available on the explicitly active package.
yu psd layer export poster.psd --id L0012 -o logo.png --json

# Validate the generated raster image; preserve and report export warnings.
yu image info logo.png --json

# Rendering and PSD mutation remain deferred.
\`\`\`

The caller only needs to understand YuTool's interface; engine-specific details remain behind the runtime.
