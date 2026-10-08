# Roadmap

This roadmap intentionally prioritizes proving YuTool's runtime model before expanding into many tool categories.

Dates are not committed here. Milestones are capability-based.

## M0 — Documentation bootstrap

**Goal:** freeze the product language and architecture boundaries before implementation.

- [x] Define YuTool / `yu` naming
- [x] Define project vision
- [x] Define capability vs engine model
- [x] Define Built-in / Managed / System engine classes
- [x] Draft CLI contract
- [x] Draft capability matrix
- [x] Define agent usage rules
- [x] Record initial engine-strategy ADR
- [x] Rename repository from `img-cli` to `yu-tool`
- [ ] Set repository description/topics
- [ ] Choose license

Exit criteria:

- contributors and coding agents can explain what YuTool is and is not;
- v0.1 command shape is documented;
- optional engines are not accidentally treated as hard dependencies.

## M1 — Rust core and first built-in capability

**Goal:** prove the smallest useful YuTool runtime without external engines.

Progress:

- [x] Rust workspace bootstrap
- [x] `yu` CLI executable
- [x] shared core result/error types
- [x] capability registry
- [x] engine registry
- [x] engine resolver
- [x] `yu doctor`
- [x] `yu capabilities`
- [x] `yu engine list`
- [x] built-in raster engine (`raster-rs`)
- [x] initial `image.info`
- [x] initial `image.resize`
- [x] JSON output contract
- [x] integration tests

The first built-in raster implementation uses the Rust `image` crate with a deliberately small default format set: PNG, JPEG, and WebP.

Performance-specialized resizing (for example a future `fast_image_resize` integration) is intentionally deferred until the public resize semantics are proven.

Exit criteria:

```bash
yu doctor
yu capabilities --json
yu image info input.png --json
yu image resize input.png --width 1024 -o output.png --json
```

work on supported platforms without ImageMagick, Python, Node, or another optional engine.

### M1 Freeze

M1 is frozen once PR #3 lands and the freeze receipt in `docs/milestones/m1-freeze.md` records the accepted CLI/protocol baseline and green cross-platform CI.

## M2 — Engine Manager foundation

**Goal:** prove optional engine lifecycle management.

Progress:

- [x] engine manifest v1 model;
- [x] platform/architecture matching foundation;
- [x] managed-engine storage layout;
- [x] download staging;
- [x] checksum/integrity verification;
- [x] safe archive extraction;
- [x] atomic activation;
- [x] installed version discovery;
- [x] active/current version state;
- [x] managed version removal;
- [x] inter-process engine mutation lock;
- [x] public managed-engine CLI;
- [x] system executable discovery foundation;
- [x] system engine version probing (ImageMagick / FFmpeg / ExifTool);
- [x] unified Built-in / Managed / System inventory;
- [x] `yu engine info`;
- [x] `yu doctor` unified inventory integration;
- [x] managed engine state diagnostics.

CLI target:

```bash
yu engine list
yu engine install --manifest <file>
yu engine versions <engine>
yu engine activate <engine> <version>
yu engine deactivate <engine>
yu engine remove <engine> <version>
yu doctor
```

Important constraint:

YuTool should not silently invoke Homebrew, apt, winget, sudo, or administrator elevation as part of an unrelated operation.

Exit criteria:

- a managed engine can be installed, verified, activated, listed, used, and removed;
- a compatible system engine can be discovered without YuTool taking ownership of it.

### M2 Freeze

M2 functional behavior is frozen at `d95ccae8cdaefd4ea63a35d39d6ec803748036e8`. PR #9 records the accepted Engine Manager, lifecycle, unified inventory, CLI, security, and schema-v1 compatibility baseline in `docs/milestones/m2-freeze.md`.

M3 may extend YuTool with PSD capabilities and new engines, but should not casually break the M2 lifecycle or inventory contracts.

## M3 — PSD strategy and runtime

**Goal:** select a practical PSD/PSB strategy based on fixtures, then prove a bounded Managed runtime and selected-layer RGBA8 PNG export.

Progress:

- [x] engine-neutral PSD spike harness;
- [x] fixture corpus schema/provenance rules;
- [x] initial synthetic malformed fixture;
- [x] PSD/PSB fixture corpus v1 (pixel layers, groups, duplicate names, text, masks);
- [x] pinned third-party fixture provenance/license receipts;
- [x] Rust / psd-tools / TypeScript candidate adapter skeletons;
- [x] machine-readable per-candidate report model;
- [x] representative redistributable PSD/PSB benchmark corpus;
- [x] first Rust-native candidate integration (rawpsd 0.2.2);
- [x] psd-tools 1.20.0 reference candidate integration (Python 3.12);
- [x] TypeScript/Node candidate integration (ag-psd 31.0.2);
- [x] candidate comparison report v1 (conformance, distribution, maintenance evidence);
- [x] controlled performance/memory benchmark report v2;
- [x] PSD engine strategy decision (ADR 0006: ag-psd Managed primary, psd-tools reference/compatibility);
- [x] PSD capability contract v1;
- [x] External Engine Protocol v1;
- [x] Managed ag-psd package prototype (Node 22.23.3 + ag-psd 31.0.2; Linux x64 / macOS arm64 / Windows x64);
- [x] runtime wiring for inspect / tree / layer list / layer info (PR #22; bounded Protocol v1, active-version capability checks, public CLI);
- [x] 8-bit RGB layer bitmap export (PR #23; RGBA8 PNG, validated private staging, atomic no-clobber publication);
- [x] M3 implementation freeze (PR #24; functional baseline `c793279e3c131cae85adf73571590ab022b2ed05`, exact package/evidence snapshots and regression guards).

Build a representative fixture corpus covering, where legally distributable:

- simple pixel layers;
- nested groups;
- duplicate layer names;
- text layers;
- masks;
- blend modes;
- effects;
- Smart Object metadata;
- higher bit depth where applicable;
- PSD and PSB;
- malformed/edge-case inputs.

Evaluate candidate implementations across:

- parse success;
- layer-tree fidelity;
- metadata coverage;
- layer export;
- rendering fidelity;
- PSD vs PSB;
- round-trip behavior where writing is supported;
- performance;
- memory usage;
- platform/distribution cost;
- maintenance risk.

Candidate categories include:

- Rust-native PSD implementations;
- psd-tools;
- TypeScript/Node PSD implementations where useful;
- other mature implementations discovered during the spike.

Deliverables:

- benchmark/conformance report;
- recommended built-in PSD engine, if one is mature enough;
- recommended compatibility/managed engine, if useful;
- explicit unsupported/partial capability list.

Strategy exit criteria are now satisfied by ADR 0006 and the M3 evidence chain.

The accepted implementation includes:

```bash
yu psd inspect design.psd --json
yu psd tree design.psd --json
yu psd layer list design.psd --json
yu psd layer export design.psd --id <id> -o layer.png --json
```

using the selected Managed ag-psd engine strategy.

### M3 Freeze

M3 functional behavior is frozen at `c793279e3c131cae85adf73571590ab022b2ed05` (PR #23 squash merge). [Freeze receipt](milestones/m3-freeze.md), [machine-readable evidence](data/m3-implementation-freeze-v1.json) and [acceptance checklist](testing/pr24-m3-freeze.md) distinguish implemented behavior from release readiness. Four read-only commands plus partial 8-bit RGB stored-layer export are accepted; rendering, non-RGB/high-bit export and mutation remain outside the baseline.

Before public distribution, separately address persistent artifacts/trusted HTTPS manifests, license decisions, packaged CLI acceptance, dependency-lock/reproducibility policy and platform signing/notarization. This is a release-readiness recommendation, not a redefinition of the M4/M5 capability milestones or permission to expand them automatically.

### v0.1 release preparation (PR #25)

Engineering preparation adds verified offline `--archive` installation, quarantine diagnostics/repeated lifecycle acceptance, locked Rust/npm dependencies and source-qualified CLI developer candidates tested after extraction. See [readiness and unaccepted gates](releasing/v0.1-readiness.md) and [test checklist](testing/pr25-release-readiness.md). This does not authorize public publishing or mark Windows root cause, licensing, signatures, minimum OS support or durable hosting as accepted. M3 historical evidence remains immutable.

### Windows lifecycle investigation (PR #26)

Separate diagnostics/reproduction work records process-exit versus pipe-EOF phases and output byte counts, adds controlled CLI cases including Windows directory occupancy, and retains finite repeated extracted-candidate runs with first-failure logs. See [investigation boundaries](reliability/windows-lifecycle.md) and [acceptance checklist](testing/pr26-windows-lifecycle.md). Native process/Job cleanup hardening and historical root-cause closure remain unimplemented; candidate promotion is prohibited.

### Explicit process cleanup result propagation (PR #28)

First lifecycle-hardening increment: explicitly settle owned cleanup, retain primary and cleanup errors, and reject success/artifact publication on cleanup failure. Controlled tests reproduce swallowed worker failure before the change. [Scope and deferred work](reliability/explicit-process-cleanup.md) and [acceptance](testing/pr28-explicit-process-cleanup.md) separate this implemented error path from creation-time Job assignment, bounded I/O cancellation, enforced whole-Job completion and historical OS5/timeout attribution. Issue #27 stays open; no release promotion.

### Independent file-resource attribution (PR #30)

This main-derived diagnostics increment refines verified batch resource users into bounded, timestamped single-file observations, with separate real-Windows calibration. It is not #29 startup integration, a directory-blocker proof, bounded runtime cleanup or release approval. See [scope](reliability/windows-resource-attribution.md) and [acceptance](testing/pr30-windows-resource-attribution.md). Historical OS5/timeout evidence and issue #27 remain open; #29's failed independent gate is not waived.

### Bounded process waiting and cancellable I/O (PR #31)

Independent runtime change on main after #30: nonblocking pipe pump, stop-and-close cancellation, bounded direct-child and Windows Job settlement, and preserved explicit cleanup/error/publication semantics. No #29 startup code or additional occupancy investigation is included. [Scope](reliability/bounded-process-io.md) and [acceptance](testing/pr31-bounded-process-io.md) distinguish tested transport bounds from hard OS-call guarantees and historical root-cause closure. Issue #27 remains open; no candidate promotion.

### Combined Windows owned / bounded runtime (PR #32)

Independent adaptation after #31: the #29 creation-time Job/handle-list contract feeds the #31 nonblocking pipe pump and shared cleanup deadline. Joint native controls cover early descendant membership plus timeout/exit, overlapping invocations and real Job-query denial. [Scope](reliability/windows-owned-bounded-runtime.md) and [acceptance](testing/pr32-windows-owned-bounded-runtime.md) retain old #29's failed gate and issue #27. No extra attribution tooling, engine changes or public candidate promotion.

### Windows startup deadline semantics (PR #34)

Independent runtime increment on #32, rebased for final acceptance after #37: classify startup overruns before protocol exchange and record bounded Windows startup-stage timings. It does not use unsafe thread termination or claim a hard wall-clock bound over `CreateProcessW`; the existing operation timeout, two-second cleanup budget and Issue #27 history remain. See [scope](reliability/windows-startup-deadline.md) and [acceptance](testing/pr34-windows-startup-deadline.md).

### Exact-directory user attribution (PR #35)

Diagnostics-only follow-up for the remaining OS5 blind spot: calibrate an exact opened-directory PID query on real Windows and attach it to the existing post-failure collector before file-level Restart Manager attribution. Because the native information class is reserved for system use, this remains investigation tooling rather than a runtime API. A directory user is not automatically a proven rename blocker. See [scope](reliability/windows-directory-attribution.md), [acceptance](testing/pr35-windows-directory-attribution.md), and [ADR 0013](decisions/0013-windows-directory-query-diagnostic-only.md); Issue #27 remains open.

### In-flight remove-window sampling (PR #36)

Test-only follow-up after a successful remove required 52 quarantine attempts / ~1317 ms and therefore escaped post-failure capture. While an explicitly instrumented `engine remove` is running, the harness samples PR #35 exact-directory identities and correlates their first/last seen times with the unchanged remove result. Three real-Windows controls cover transient-success, persistent-failure and sibling isolation. See [scope](reliability/windows-remove-window-sampling.md) and [acceptance](testing/pr36-windows-remove-window-sampling.md); this does not change the production retry loop or prove a causal rename blocker.

### Distribution notices and isolated acceptance (PR #33)

Originally developed after #32 and rebased for final acceptance onto `main@f0a3f052e52ce9ba89c59bbebbce813b3b84410e` after #34/#37. Distribution preparation packages conservative Cargo source notices, a hashed dependency inventory and usage guidance with new CLI candidates, then checks core/offline PSD workflows in a fresh process environment. See [scope](releasing/distribution-readiness.md) and [acceptance](testing/pr33-distribution-readiness.md). This is not legal clearance, clean-VM/signing/minimum-OS acceptance or public release; Issue #27 stays open.

### v0.1 Final Release Gate / RC Freeze

The release-candidate freeze starts after #33, #34 and #37 are integrated. The freeze does not authorize a public release: it freezes the candidate source, requires post-merge exact-`main` CI / Managed Package / Windows Lifecycle evidence, and moves any further code change back through the full release gate. See [final release gate](releasing/v0.1-final-gate.md) and [RC freeze acceptance](testing/v0.1-rc-freeze.md).

Only release-blocking fixes may change the frozen candidate line. Feature work continues after the v0.1 decision rather than being folded into the RC.

## M4 — Safe mutation

**Goal:** introduce modifications without compromising source-file safety.

M4a is developed on `feature/m4a-raster-operations`, based on the
technical-v0.1 frozen main commit `5526dc4465c4e79ad7f2757d06b3b1693642b8af`.
It adds crop, right-angle rotate and format convert to the built-in raster
engine, with explicit no-clobber publication and CLI integration tests.
This is **post-v0.1 feature development**, not a change to the frozen RC.
M4b safe replacement and PSD mutation remain separate follow-ups beyond M4a.

M4b-1 builds on the merged M4a `develop@990a237be4a1505c7d49dd87c460560c31a63768`.
It implements image-only `--dry-run` preflight and structured dry-run results
without writing outputs; safe overwrite/replacement, durable approvals, and
PSD mutation remain separate follow-ups. The frozen v0.1 `main` does not
receive M4 changes.

M4b-2 follows `develop@fbe1bf9719a5b42c1035c29edb1b0e7ae958a069`:
explicit version-bound replacement on the four built-in image operations,
same-directory stage/publish, double SHA-256 validation, safe cleanup and
YuTool-cooperating writer serialization. There is no portable filesystem
compare-and-swap and the known race against uncooperative writers is stated
in the CLI specification. The `main` RC and signing/release gates remain
untouched; GitHub Actions are not a required development gate while quota
is unavailable.

M4b-3 follows `develop@b5dccf94f89ff5b568b2cd6c6079023fd2894daf`:
verify encoded staging, reopen/hash/decode the published destination, and
return a typed receipt with old/new SHA-256, bytes, dimensions and format.
Dry-run stays a plan. Errors after publication never claim success or
automatically roll back. This feature is isolated from v0.1 frozen `main`.

### M4c-1 — PSD mutation feasibility spike

Based on `develop@9c9ea881585b7b7c1ec54f4b6c73fb507a480285`,
the pinned Node 22.23.3 / ag-psd 31.0.2 probe ran **48** source-preserving
no-op/rename/visibility/opacity cases across 12 PSD/PSB fixtures.
Forty saved and reparsed, eight high-bit writes failed closed, and the
experiment surfaced unintended layer-effect/resource changes and a no-op
cached-composite mismatch. Visibility/opacity can also leave a stale
composite preview. Four simple-document/group/duplicate-name rename
cases pass the measured metadata checks, but **none authorizes a public PSD
mutation capability**. This is a negative safety gate with a narrow research
candidate, not a completed PSD editor.

See the [M4c-1 investigation](psd-mutation-feasibility-m4c1.md) and
[machine evidence](data/m4c1-psd-mutation-evidence-v1.json).

### M4c-2 — PSD safe-mutation scope / contract freeze

Starting from `develop@ff61d9aa975e9decc5f19bd555e0d916e529060d`,
ADR 0014 freezes a **research-only, default-deny PSD mutation policy**:
exact SHA-256 identity for four prior reviewed fixtures (simple PSD/PSB,
group and duplicate names), 8-bit RGB, bounded source, canonical layer
ID + expected old name, source byte version binding, new path only and
read-only plan. Only **rename** is a research candidate. Arbitrary user
PSD files, unknown/opaque resources, complex effects, masks, text,
Smart Objects, high bit depth, visibility/opacity and general writes
remain **blocked**. No `yu psd` public write command or Managed protocol
change is authorized.

See [ADR 0014](decisions/0014-psd-mutation-input-scope-freeze.md),
[machine policy](data/psd-safe-mutation-scope-m4c2-v1.json) and
[acceptance](testing/m4c2-psd-input-scope-freeze.md).
Next: independent raw-block scanner and full-fidelity editor acceptance
before expanding any input class beyond the exact reviewed research fixtures.

### M4c-3 — PSD/PSB byte-level metadata inventory

Starting at `develop@00d0c2fb2b67f3fc5561374498d2fb3365358f92`,
the read-only standalone research scanner inventories raw Image Resources,
layer records, Additional Layer Information, PSB long-length tagged blocks
and opaque section boundaries with offsets and byte-level fingerprints.
Unknown tags/resource IDs are surfaced rather than dropped. In the
committed corpus 13 valid PSD/PSB inputs scan, one malformed file is
rejected, and **11/13** valid inputs contain unknown resource IDs or tags.
All scans report `mutation_authorized=false` and `safe_to_rewrite=false`,
including the two fixtures without unknown blocks. A recognizable block
does not prove that any editor can preserve its contents.

See [ADR 0015](decisions/0015-psd-raw-block-inventory-boundary.md),
[inventory evidence](data/psd-raw-block-inventory-m4c3-v1.json) and
[acceptance](testing/m4c3-psd-raw-block-inventory.md). M4c-4 should
investigate nested layer records and independent preservation/fidelity
before any policy expansion or PSD write CLI.

Candidate capabilities:

- image crop/rotate/convert;
- PSD layer rename where supported;
- PSD show/hide where supported;
- opacity changes where supported;
- safe output replacement;
- `--dry-run`;
- structured change summary;
- post-operation validation.

Any PSD mutation capability must be gated by actual engine conformance results.

## M5 — YuTool Manager

**Goal:** provide a small GUI for runtime/engine management.

Initial GUI scope:

- installed engines;
- available managed engines;
- version;
- provider class;
- capability list;
- install/update/remove;
- health state;
- disk usage;
- license/source metadata.

The first Manager is **not** an image editor.

Preferred direction: Tauri sharing the Rust core with the CLI.

## After the runtime is proven

Only after the engine/runtime architecture is stable should YuTool expand into additional capability families.

Potential areas include:

```text
PDF
media / FFmpeg
metadata
SVG/vector
OCR
archives
documents
RAW
```

Each new family should begin with:

1. a capability contract;
2. engine candidates;
3. conformance fixtures;
4. an explicit scope decision.

## Non-goals for the first release

Do not make v0.1 responsible for:

- every image format;
- Photoshop-level PSD editing;
- full PDF tooling;
- video/audio transcoding;
- OCR;
- a general plugin marketplace;
- remote/cloud execution;
- an image-editing GUI.

The first release should prove that **one lightweight runtime can safely discover, resolve, execute, and report local capabilities through `yu`.**
