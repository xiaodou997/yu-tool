# Architecture

## Overview

YuTool separates user intent from implementation details.

\`\`\`text
Developers / Scripts / AI Agents
              │
              ▼
            yu CLI
              │
              ▼
      Capability Registry
              │
              ▼
        Engine Resolver
              │
    ┌─────────┼─────────┐
    ▼         ▼         ▼
 Built-in   Managed    System
 Engines    Engines    Engines
\`\`\`

The core should understand capabilities and engine contracts. It should not embed backend-specific behavior directly into command parsing.

## Terminology

### Capability

A **capability** represents an operation YuTool exposes.

Examples:

\`\`\`text
image.info
image.resize
image.crop
psd.inspect
psd.tree
psd.layer.export
\`\`\`

Capabilities are the stable surface presented to callers.

### Engine

An **engine** implements one or more capabilities.

Examples may include:

- Rust image libraries;
- Rust PSD implementations;
- psd-tools;
- ImageMagick;
- libvips;
- ExifTool.

An engine may support only a subset of a capability family.

### Engine provider class

YuTool recognizes three installation classes.

#### Built-in

Compiled into the YuTool distribution.

Advantages:

- no external installation;
- predictable availability;
- easiest path for common operations.

#### Managed

Downloaded, installed, versioned, and removed by YuTool.

Managed engines should live in YuTool-controlled storage and should not require users to modify system package managers.

#### System

Already installed on the host machine.

Examples include an existing \`magick\`, \`ffmpeg\`, or other compatible binary found in a known location or on \`PATH\`.

YuTool may use system engines but does not own their lifecycle.

## Main components

### CLI

Responsibilities:

- parse commands;
- select output format;
- map errors to stable exit codes;
- pass requests to the core.

The CLI should not directly execute backend commands.

### Capability Registry

Responsibilities:

- enumerate known capabilities;
- expose capability metadata;
- report support state;
- provide requirements for engine selection.

This powers:

\`\`\`bash
yu capabilities
yu capabilities --json
\`\`\`

### Engine Registry

Responsibilities:

- enumerate known engine definitions;
- report installation state;
- report versions;
- expose supported capabilities;
- expose source: built-in, managed, or system.

This powers:

\`\`\`bash
yu engine list
\`\`\`

### Engine Resolver

The resolver chooses an implementation for a requested operation.

Initial resolution priority:

1. explicit user-selected engine;
2. suitable built-in engine;
3. suitable managed engine;
4. suitable system engine;
5. return a structured unsupported/unavailable error.

This order may become policy-configurable later.

Resolution should consider more than availability. An engine may be installed but unsuitable for a specific file, format, bit depth, operation, or requested fidelity.

### Engine Manager

Responsibilities for managed engines:

- discover available engine packages;
- download;
- verify;
- install atomically;
- activate;
- update;
- remove;
- report disk usage and version.

It must not silently elevate privileges or install packages through Homebrew/apt/winget without explicit future design approval.

### Doctor

\`yu doctor\` diagnoses runtime state.

It should answer questions such as:

- Is the YuTool installation healthy?
- Which engines were discovered?
- Which managed engines are broken?
- Which engine versions are active?
- Are required runtime components missing?

### Desktop Manager

A future **YuTool Manager** GUI should reuse the same Rust core as the CLI.

Its first purpose is engine/runtime management, not image editing.

\`\`\`text
              yu-core
                │
       ┌────────┴────────┐
       ▼                 ▼
     yu CLI       YuTool Manager
                       Tauri
\`\`\`

## Rust-first core

The preferred implementation direction is a Rust workspace.

A tentative structure:

\`\`\`text
crates/
├── yu-core
├── yu-cli
├── yu-engine-api
├── yu-engine-manager
├── yu-capability-image
└── yu-engine-image-rs

apps/
└── manager
\`\`\`

This is a boundary proposal, not a frozen crate list.

## Multi-language engines

External engines are intentionally language-agnostic.

A provider can wrap:

- Rust libraries;
- C/C++ native applications;
- Python runtimes/packages;
- TypeScript/Node packages;
- standalone executables.

The engine contract must hide runtime-specific invocation from capability callers.

## External engine process protocol

External language runtimes cross a versioned process boundary rather than being called ad hoc from capability code.

Protocol v1 is defined in `docs/external-engine-protocol-v1.md` and `yu-engine-api`.

The v1 model is:

```text
YuTool Rust runtime
        │
        ├─ spawn selected engine entrypoint
        ├─ JSON request → stdin
        ├─ JSON response ← stdout
        └─ diagnostics ← stderr
```

One process handles one request in v1. Valid capability errors are represented in the JSON response; non-zero process exit is reserved for transport/process failure.

The protocol envelope is capability-neutral. Domain crates such as `yu-capability-psd` own the typed payload/result contracts.

This keeps:

- backend-specific runtime details outside the CLI contract;
- request IDs correlated across process boundaries;
- stdout machine-only;
- paths and user values out of shell interpolation;
- engine replacement possible without changing public capability schemas.

## Read-only PSD runtime (PR #22)

`yu-cli` parses commands and renders results. `yu-runtime-psd` owns PSD routing and the bounded external process adapter. `yu-capability-psd` and `yu-engine-api` retain their frozen v1 contracts; `yu-engine-manager` retains package ownership and lifecycle.

```text
yu psd <read-only command>
  -> yu-runtime-psd
  -> EngineManager::active_command()
  -> exact active version capability check
  -> fixed entrypoint + argv + package working directory
  -> Protocol v1 request/response
  -> typed PSD result and semantic validation
  -> schema-v1 success/error envelope
```

`ManagedEngineCommand.capabilities` is additive and belongs to the same version snapshot as its command. Inventory capabilities remain the union of valid installed versions for M2 compatibility; they are not execution authorization. A concurrent activation cannot silently change a resolved command's reported version. Concurrent removal/filesystem failure can still make execution fail; there is no automatic retry against a different version.

The runtime canonicalizes caller input and engine paths before switching the child working directory. It clears `NODE_OPTIONS` and `NODE_PATH`, uses stdin JSON rather than interpolated shell input, and bounds request/stdout/stderr sizes. Three I/O workers avoid pipe backpressure deadlock; the operation deadline includes waiting for EOF. Unix process groups and Windows Job Objects provide owned-process cleanup, including ordinary children that retain pipe handles. This is lifecycle containment of a trusted installed engine, not a sandbox against deliberately escaping executable code; package provenance still matters.

The four exposed capabilities are the intersection of wired read-only operations and the active version's declarations. No probing or installation occurs during effective capability enumeration. JSON errors may add selected engine metadata without changing existing fields or error codes.

## Managed runtime layout

A possible managed-runtime layout:

\`\`\`text
<yu-data>/
├── engines/
│   ├── imagemagick/
│   │   └── <version>/
│   ├── psd-tools/
│   │   └── <version>/
│   └── ...
├── cache/
└── state/
\`\`\`

The exact platform directories should follow OS conventions rather than hard-coding \`~/.yu\` everywhere.

## Structured results

Capability execution should return a structured internal result before presentation.

Conceptually:

\`\`\`json
{
  "schema_version": "1",
  "operation": "image.resize",
  "engine": {
    "id": "raster-rs",
    "version": "..."
  },
  "result": {},
  "warnings": []
}
\`\`\`

The human renderer and JSON renderer should consume the same internal result.

## Error model

Errors should be classified rather than forwarded as raw backend failures.

Initial categories:

- invalid argument;
- invalid/corrupt input;
- unsupported capability;
- engine unavailable;
- engine incompatible;
- execution failure;
- output conflict;
- verification failure.

Backend diagnostics can be attached as debug details without becoming the public error contract.

## Security boundaries

External engine execution is a trust boundary.

Requirements include:

- no shell-string interpolation for user-controlled values;
- explicit argv construction;
- bounded/cancellable execution where practical;
- controlled environment variables;
- validated download sources for managed engines;
- checksum/signature verification where available;
- atomic activation of managed engine versions.

## PSD engine strategy

ADR 0006 resolves the initial PSD engine question:

- v0.1 has no Built-in PSD engine;
- ag-psd 31.0.2 is the preferred Managed PSD engine;
- its production package should carry a private Node.js 22 runtime or equivalent self-contained entrypoint;
- psd-tools remains an independent reference/explicit compatibility implementation;
- PSD engines do not silently fail over between implementations;
- high-bit-depth layer export is outside the v0.1 contract until normalization semantics are accepted.

## Open architecture questions

The following should still be resolved through implementation work or later ADRs:

- exact self-contained Managed package layout for language-runtime engines;
- catalog/update/signing policy for managed runtime bundles;
- stable third-party engine process protocol;
- high-bit-depth normalized/native export contract;
- render-fidelity contract;
- plugin ABI/process protocol for third-party engines.
