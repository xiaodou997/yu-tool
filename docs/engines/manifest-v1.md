# Managed Engine Manifest v1

> Status: **M2 foundation draft**

YuTool-managed engines are described by a manifest. The manifest answers:

- what engine/version is being installed;
- which YuTool capabilities it provides;
- which platform package applies;
- where the package comes from;
- how the artifact is verified;
- which relative entrypoint becomes executable after installation.

## Schema version

```json
{
  "schema_version": "1"
}
```

M2 starts with schema version `1`.

## Example

```json
{
  "schema_version": "1",
  "id": "imagemagick",
  "display_name": "ImageMagick",
  "version": "7.x.y",
  "capabilities": [
    "image.advanced"
  ],
  "packages": [
    {
      "target": {
        "os": "macos",
        "arch": "aarch64"
      },
      "url": "https://example.invalid/imagemagick-macos-aarch64.zip",
      "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "archive": "zip",
      "entrypoint": "bin/magick",
      "args": []
    }
  ]
}
```

The URL above is illustrative only. No ImageMagick package source is selected by this document.

## Engine ID

`id` is a stable machine identifier.

Allowed characters:

```text
a-z
0-9
.
_
-
```

Examples:

```text
imagemagick
psd-tools
libvips
ffmpeg
```

## Targets

A package target consists of:

```json
{
  "os": "macos",
  "arch": "aarch64"
}
```

YuTool initially compares exact Rust platform strings from `std::env::consts`.

Common examples:

| OS | Architecture |
| --- | --- |
| `macos` | `aarch64` |
| `macos` | `x86_64` |
| `linux` | `x86_64` |
| `linux` | `aarch64` |
| `windows` | `x86_64` |

A manifest must not contain duplicate packages for the same target.

## Verification

Every managed package requires a SHA-256 digest.

YuTool verifies the downloaded artifact against this digest before extraction or activation. A mismatch aborts installation and the final engine version directory is never activated.

## Archive kinds

Manifest v1 reserves:

- `raw`
- `zip`
- `tar_gz`

The installer supports all three kinds. Archive extraction is bounded by file-count and total extracted-byte limits.

## Entrypoint safety

`entrypoint` must be a relative path inside the installed engine version directory.

Rejected examples:

```text
/opt/tool/bin/tool
../escape
../../tool
```

This prevents a manifest from defining an entrypoint outside YuTool-managed storage.

## Fixed entrypoint arguments

M3 adds an **optional additive** `args` field to each package:

```json
{
  "entrypoint": "runtime/node",
  "args": ["engine/ag_psd_protocol.cjs"]
}
```

Rules:

- omitted `args` means an empty argv and preserves the frozen M2 behavior;
- arguments are package-controlled fixed strings, not user input;
- at most 64 arguments are accepted;
- total encoded argument text is limited to 64 KiB;
- NUL bytes are rejected;
- arguments are passed directly to the executable without shell interpolation;
- `args` are persisted into `.yu-install.json`;
- the runtime working directory for an active Managed command is the installed engine version root.

This makes self-contained language-runtime engines possible without introducing shell wrapper scripts.

## Managed storage

YuTool owns only its private managed-engine storage.

Conceptually:

```text
<yu-data>/
├── engines/
│   └── <engine-id>/
│       └── <version>/
│           └── ...
├── cache/
└── state/
```

Default roots:

- macOS: `~/Library/Application Support/YuTool`
- Linux: `$XDG_DATA_HOME/yu-tool`, otherwise `~/.local/share/yu-tool`
- Windows: `%LOCALAPPDATA%\YuTool`

`YU_DATA_HOME` overrides the default root on every platform.

## Managed engine state

Given a valid manifest and current target:

- no matching target package → `incompatible`
- version directory absent → `not_installed`
- version directory exists but entrypoint is missing → `broken`
- expected entrypoint exists → `ready`

These states reuse YuTool's existing M1 `EngineState` model.

## System engines

System engines are separate from managed engines.

YuTool may probe `PATH` for known executable names, but discovering a binary does not transfer ownership to YuTool.

System packages must never be deleted by `yu engine remove`.


## Installation pipeline

Managed installation follows this order:

```text
manifest validate
      ↓
target select
      ↓
download to YuTool cache/staging
      ↓
SHA-256 verify
      ↓
safe extract into staging payload
      ↓
entrypoint validation
      ↓
mark entrypoint executable when required
      ↓
atomic rename into engines/<id>/<version>
```

The final version directory is not created until all earlier steps succeed.

### Archive safety

YuTool rejects:

- absolute archive paths;
- parent-directory traversal;
- backslash-based paths in managed archives;
- ZIP symlinks;
- TAR symlinks, hardlinks, devices, and other non-file/non-directory entries;
- archives exceeding configured extraction limits.

### Download safety

The built-in HTTP downloader:

- accepts only `https://` URLs;
- streams to a staging file rather than buffering the full artifact;
- enforces a download-size limit;
- uses a bounded request timeout;
- verifies SHA-256 before extraction.

No managed artifact is activated before integrity verification succeeds.
