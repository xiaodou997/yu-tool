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
