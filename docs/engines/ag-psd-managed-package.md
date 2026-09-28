# Managed ag-psd Engine Package Prototype

PR #21 turns the M3 engine strategy and Protocol v1 contract into an installable Managed Engine package prototype.

## Current export-capable package (PR #23)

PR #23 adds the distinct package version `31.0.2+node22.23.3.yu2`, retaining ag-psd 31.0.2, private Node 22.23.3 and the v1 protocols. It declares the four read-only operations plus `psd.layer.export` for stored 8-bit RGB layer bitmaps as RGBA8 PNG. Users must explicitly install and activate the new package; an old active package does not acquire export support merely because the CLI was updated.

The current builder/source matrix emits this new package. Its Managed Package workflow runs the original lifecycle smoke and both actual-CLI read-only/export tests. Exact new package hashes and run outcomes belong to its generated metadata and PR acceptance receipt; **the PR #21 hashes below do not describe `.yu2`**. Distribution remains prototype-only with placeholder HTTPS URLs.

See [PR #23 export semantics and testing](../testing/pr23-psd-layer-export.md).

## Historical PR #21 frozen bundle

```text
engine:          ag-psd
ag-psd:         31.0.2
private runtime: Node.js 22.23.3
engine version:  31.0.2+node22.23.3
protocol:        External Engine Protocol v1
PSD contract:    PSD Capability Contract v1
```

The runtime is private to the Managed Engine package. YuTool does not require or mutate a system Node.js installation.

## Validated prototype targets

| YuTool target | GitHub runner | Node upstream archive |
| --- | --- | --- |
| `linux/x86_64` | `ubuntu-latest` | `node-v22.23.3-linux-x64.tar.xz` |
| `macos/aarch64` | `macos-latest` | `node-v22.23.3-darwin-arm64.tar.xz` |
| `windows/x86_64` | `windows-latest` | `node-v22.23.3-win-x64.zip` |

The exact Node URLs and official SHA-256 receipts are committed in:

```text
packaging/ag-psd-engine/node-runtime-sources-v1.json
```

Additional targets are not claimed by this prototype until they receive the same package/install/execute smoke.

## Package layout

Each ZIP has the same logical shape:

```text
runtime/
└── node[.exe]

engine/
├── ag_psd_protocol.cjs
├── yu-engine.json
└── node_modules/
    ├── ag-psd/
    ├── base64-js/
    └── pako/

licenses/
└── node-LICENSE
```

The npm package directories retain their own license/package metadata.

The package deliberately excludes npm itself, development dependencies, canvas, TypeScript, and package-manager caches.

## Managed manifest command

Manifest v1 now supports an additive optional fixed-argv field:

```json
{
  "entrypoint": "runtime/node",
  "args": ["engine/ag_psd_protocol.cjs"]
}
```

On Windows the entrypoint is `runtime/node.exe`.

The execution contract is:

```text
working directory = engines/<id>/<version>/
executable        = verified installed entrypoint
argv              = verified installed metadata args
```

No shell is involved.

Existing M2 manifests with no `args` deserialize exactly as an empty argv and retain their previous behavior.

## Build provenance

The package builder:

1. downloads the pinned official Node archive;
2. verifies the committed official SHA-256;
3. extracts only the Node executable and Node license;
4. verifies exact installed npm package versions;
5. copies only ag-psd/base64-js/pako package trees;
6. copies the Protocol v1 adapter;
7. writes an internal engine receipt;
8. creates a deterministic ZIP;
9. calculates the final package SHA-256;
10. emits a single-target Engine Manifest v1 and package metadata.

Builder:

```text
tools/build_ag_psd_managed_package.py
```

## CI acceptance path

`.github/workflows/ag-psd-managed-package.yml` runs independently from the generic Rust/PSD Spike gates.

For each validated target it performs:

```text
build verified package
       ↓
EngineInstaller
       ↓
SHA-256 verify
       ↓
safe ZIP extract
       ↓
write .yu-install.json
       ↓
explicit activate
       ↓
active_command()
       ↓
private Node + fixed adapter argv
       ↓
Protocol v1 psd.inspect
       ↓
managed inventory = ready
       ↓
deactivate + remove
```

This smoke proves that the M2 lifecycle, PR #20 process protocol, and the private runtime package operate as one path.

## Prototype distribution boundary

PR #21 CI artifacts are prototype build evidence, not a public engine catalog.

The generated manifests intentionally use an `https://example.invalid/` package base URL. They validate the Engine Manifest contract and can be installed by the CI local-package downloader, but are not advertised as remotely installable release manifests.

A later release/catalog step must publish the exact accepted ZIPs to stable HTTPS URLs and replace the prototype URLs without changing package contents.

## Capabilities declared by the historical PR #21 prototype

The original PR #21 package advertised only these four capabilities:

- `psd.inspect`
- `psd.tree`
- `psd.layer.list`
- `psd.layer.info`

That original package does **not** advertise `psd.layer.export`. The current `.yu2` package adds it explicitly; discovery and execution use the activated version's declarations.


## Historical PR #21 accepted prototype receipt

The accepted three-target prototype is frozen in:

```text
docs/data/ag-psd-managed-manifest-prototype-v1.json
docs/data/ag-psd-managed-package-prototype-v1.json
```

Evidence source:

```text
Workflow:    Managed ag-psd Package
Run ID:      36297203908
Source head: 2592275cf9b965ecb575faed9a670814fb7693a9
```

Accepted package ZIPs:

| Target | Package bytes | Package SHA-256 |
| --- | ---: | --- |
| `linux/x86_64` | 49,770,553 | `80b2edba1be82146fca6367d3062b3583eeb2b937cb849072be252b1c364b83b` |
| `macos/aarch64` | 42,987,682 | `46e16561758b49577b1f2f624f2af4efc28b120fd8d39f02b332fcf68e59e971` |
| `windows/x86_64` | 39,179,579 | `0ff9802987ebaa47dc2a082968c12d1c9dd640ed3a087076d8d1a707b86ebb12` |

All three targets independently completed the full package → install → activate → private runtime → Protocol v1 → inventory ready → cleanup smoke.

The SHA values above are the hashes of the inner Managed Engine ZIPs consumed by EngineInstaller. GitHub Actions artifact digests are recorded separately in the machine-readable receipt because Actions wraps the uploaded files in its own artifact container.
