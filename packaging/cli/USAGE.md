# YuTool developer candidate

This archive is for development acceptance, not an approved public v0.1 release.
It is unsigned/unnotarized unless a separate receipt explicitly states otherwise.
Do not bypass operating-system security controls to run an untrusted binary.
Verify archive provenance and SHA-256 through a trusted channel; a checksum
packaged alongside an untrusted archive is not authentication.

## Core: no optional engine required

Run `bin/yu` (`bin/yu.exe` on Windows) from this directory, or use its absolute path:

```sh
bin/yu --version
bin/yu doctor --json
bin/yu capabilities --json
bin/yu image info input.png --json
bin/yu image resize input.png --width 1024 -o resized.png --json
```

Use a fresh `YU_DATA_HOME` for testing. Mutations write a NEW output path by default.
The CLI itself does not need Python, Cargo, npm or a system Node installation.
The separate acceptance harness needs Python, which is a test dependency only.

## Optional PSD engine: explicit offline input

Obtain a trusted platform-matched engine manifest and its matching INNER archive.
An Actions artifact ZIP is an outer transport container: unpack that first.

```sh
bin/yu engine install --manifest manifest.json --archive engine.zip --json
bin/yu engine activate ag-psd 31.0.2+node22.23.3.yu2 --json
bin/yu psd inspect design.psd --json
bin/yu psd layer list design.psd --json
bin/yu psd layer export design.psd --id L0001 -o layer.png --json
bin/yu engine deactivate ag-psd --json
bin/yu engine remove ag-psd 31.0.2+node22.23.3.yu2 --json
```

Installation is not activation. The engine carries a private Node runtime.
Choose an actual bitmap layer ID from the layer listing, not an assumed layer name.
Export is the stored 8-bit RGB layer bitmap as RGBA8 PNG, NOT Photoshop rendering:
no masks, effects, blend-mode composition or ICC conversion. Existing outputs are
not overwritten. Persistent Windows quarantine failure keeps the installed version
and reports an error; do not force-unlock another application's resources.

## Notices and remaining release gates

`THIRD-PARTY-NOTICES.txt` preserves discovered dependency notice texts.
`dependency-inventory.json` records their upstream declarations, source and hashes.
It is a conservative Cargo normal/build dependency inventory, not an exact linker
map or legal approval. Native embedded code, toolchain/runtime notices, project
license selection and final redistribution review remain separate acceptance work.
Optional engine notices are inside its own archive, including `licenses/node-LICENSE`.

Issue #27 retains historical Windows OS5/uninstall and inspect-timeout risk.
Minimum supported OS, clean-machine validation, signing, durable downloads and an
explicit release decision remain unaccepted. Developer candidates always state
`public_release_ready: false`.
