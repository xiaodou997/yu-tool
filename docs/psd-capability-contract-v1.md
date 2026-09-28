# PSD Capability Contract v1

This document freezes the backend-independent PSD/PSB capability model selected after M3.

Machine-readable snapshot:

```text
docs/data/psd-capability-contract-v1.json
```

Rust types:

```text
crates/yu-capability-psd
```

## Capability IDs

v1 reserves:

```text
psd.inspect
psd.tree
psd.layer.list
psd.layer.info
psd.layer.export
```

The first four have reference-engine conformance coverage in PR #20.

`psd.layer.export` preserves its frozen request/result schema. PR #23 implements the 8-bit RGB stored-bitmap subset through the separately versioned Managed package.

## Stable layer identity

Layer names are presentation data, not selectors.

YuTool assigns backend-independent IDs using the canonical logical layer tree:

1. roots remain in source/document order;
2. traversal is depth-first pre-order;
3. a group receives its ID before its descendants;
4. numbering starts at one;
5. IDs are rendered as `L` plus a decimal number padded to at least four digits.

Examples:

```text
L0001
L0002
...
L9999
L10000
```

For:

```text
Group
├─ Child A
└─ Child B
Root Pixel
```

the IDs are:

```text
Group      L0001
Child A    L0002
Child B    L0003
Root Pixel L0004
```

Duplicate names do not change identity and remain legal.

The IDs are stable for one canonical logical tree. They are not persistent Photoshop object UUIDs and are not promised to survive arbitrary document mutation/reordering.

## Document model

`PsdDocumentInfo` contains:

- format: PSD or PSB;
- width / height;
- channel count;
- bits per channel;
- color mode;
- logical layer count;
- maximum logical tree depth.

Backend-private IDs or parser-specific structures are excluded.

## Layer model

Each layer exposes:

- stable YuTool layer ID;
- optional parent ID;
- depth;
- name;
- normalized kind;
- visibility;
- optional bounds;
- pixel-mask presence;
- vector-mask presence;
- logical child count.

Normalized kinds are:

- `group`;
- `pixel`;
- `text`;
- `shape`;
- `smart_object`;
- `unknown`.

The contract deliberately does not expose an ag-psd/rawpsd/Photoshop internal layer ID as the public selector.

## Operation contracts

### psd.inspect

Request:

```json
{"input_path":"/path/design.psd"}
```

Result:

```json
{
  "contract_version":"1",
  "document":{}
}
```

### psd.tree

Returns the normalized nested layer tree.

### psd.layer.list

Returns the same logical layers flattened in canonical ID order.

### psd.layer.info

Takes one canonical `layer_id` and returns that layer's normalized summary.

A missing or malformed ID is an `INVALID_ARGUMENT` result. Engines must not silently fall back to a name match.

### psd.layer.export

v1 freezes:

```text
input_path
layer_id
output_path
```

and the result fields for:

```text
RGBA8
PNG
width / height
layer_id
output_path
```

The accepted v0.1 strategy is still:

- only 8-bit source documents for layer export;
- normalized RGBA8 pixels;
- PNG output;
- 16/32-bit layer export unsupported until a separate high-bit contract is accepted.

### Export implementation boundary (PR #23)

The first implementation requires 8-bit RGB input and a selected layer with non-empty stored RGB channels. Group composition, other color modes, high-bit export and rendering are unsupported. Output contains the layer's stored pixels and transparency, not applied masks, opacity, effects or color-profile conversion; a warning is returned on success.

The Rust host supplies a private staging `output_path` to the engine, requires the exact path and selected ID in the response, validates a complete static RGBA8 PNG, and publishes it without replacing any existing destination. The public CLI result reports the final destination instead. This is internal staging of the same frozen path-based protocol, not an additional public transport.

The export-capable Managed package is `31.0.2+node22.23.3.yu2`. Installing it does not activate it, and the earlier package is not silently replaced. Limits and filesystem requirements are documented in `docs/cli-spec.md`.

## Paths

Protocol v1 represents paths as JSON strings.

The Rust execution layer rejects a path that cannot be represented losslessly for the selected external engine rather than silently replacing characters.

The engine receives paths as data in stdin JSON; paths are never shell-interpolated.

## Current reference adapter

PR #20 adds:

```text
crates/yu-psd-spike/adapters/typescript/ag_psd_protocol.cjs
```

It pins:

```text
Node.js 22
ag-psd 31.0.2
```

and implements:

- inspect;
- tree;
- layer list;
- layer info;
- 8-bit RGB stored-layer bitmap export (added in PR #23).

PR #20 originally reserved export without implementing it. PR #23 adds explicit bitmap semantics and conformance checks without broadening the contract to rendering or high-bit conversion.
