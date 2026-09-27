# External Engine Protocol v1

External Engine Protocol v1 is the process boundary between the Rust YuTool runtime and optional Managed/System engines implemented in another runtime.

The Rust model lives in `yu-engine-api`.

## Transport

v1 is deliberately one-shot:

```text
YuTool
  │
  ├─ spawn engine entrypoint
  ├─ write exactly one JSON request to stdin
  ├─ close stdin
  │
  ├─ read exactly one JSON response from stdout
  └─ collect stderr as diagnostics
```

There is no daemon/session state in protocol v1.

YuTool must construct argv directly. User-controlled values are carried inside the JSON payload, not interpolated into a shell command.

## Request envelope

```json
{
  "protocol_version": "1",
  "request_id": "req-0001",
  "capability": "psd.inspect",
  "payload": {}
}
```

Rules:

- `protocol_version` must be exactly `"1"`;
- `request_id` is 1–128 safe ASCII characters and is used only for correlation;
- `capability` is a lowercase capability ID such as `psd.inspect`;
- `payload` is capability-specific.

## Response envelope

Success:

```json
{
  "protocol_version": "1",
  "request_id": "req-0001",
  "status": "ok",
  "result": {},
  "warnings": []
}
```

Business/capability error:

```json
{
  "protocol_version": "1",
  "request_id": "req-0001",
  "status": "error",
  "error": {
    "code": "UNSUPPORTED_CAPABILITY",
    "message": "..."
  },
  "warnings": []
}
```

The response must echo the exact request ID.

v1 protocol error codes intentionally align with the public YuTool error model where applicable:

- `INVALID_ARGUMENT`;
- `INVALID_INPUT`;
- `UNSUPPORTED_CAPABILITY`;
- `ENGINE_INCOMPATIBLE`;
- `EXECUTION_FAILED`.

## Exit-code policy

A syntactically valid Protocol v1 request receives a Protocol v1 response and the engine process exits with code `0`, even when the response status is `error`.

Non-zero exit codes are transport/process failures, not normal capability errors.

The reference adapter uses exit code `2` when it cannot establish a valid protocol exchange, for example:

- malformed JSON;
- unsupported protocol version;
- invalid request envelope header;
- oversized request.

This distinction prevents subprocess exit codes from becoming a second competing business-error protocol.

## stdout / stderr

stdout is protocol-only.

An external engine must never emit logs, progress, banners, package warnings, or debug output to stdout.

stderr is diagnostic-only and is not part of the stable JSON result contract.

## Compatibility

Protocol version changes are explicit.

Within v1, receivers should tolerate unknown additive JSON fields unless a capability-specific contract explicitly says otherwise.

Removing fields, changing field meaning, or changing transport/exit semantics requires a protocol-version decision.

## Reference conformance

PR #20 includes an ag-psd reference adapter and executes it on Ubuntu, macOS, and Windows through the existing PSD Spike workflow.

The reference adapter is not the final Managed package. PR #21 will package the same protocol boundary into YuTool-owned engine storage.
