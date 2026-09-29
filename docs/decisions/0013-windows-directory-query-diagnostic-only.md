# ADR 0013 — Keep exact-directory native query diagnostic-only

## Status

Accepted for PR #35 investigation tooling. It does not approve public release or close Issue #27.

## Context

YuTool's existing Windows failure-time capture can identify regular-file users, but a directory-only handle can block a version-directory rename without producing a Restart Manager file witness. The preserved investigation experiment uses `NtQueryInformationFile` with `FileProcessIdsUsingFileInformation` to observe PIDs reported against the exact opened directory object.

Microsoft documents `FileProcessIdsUsingFileInformation` (value 47) as **reserved for system use**. Its behavior is therefore not an appropriate compatibility contract for YuTool's public CLI or normal runtime path.

## Decision

Use this information class only inside the bounded Windows diagnostic collector and its calibration tests.

The production Rust runtime, engine manager and public CLI must not depend on it for correctness, removal, cleanup or engine execution. Unsupported/denied/malformed results stay explicit evidence gaps. The tool must not unlock resources, change ACLs/privileges, terminate external processes, or widen removal retries.

A reported directory user is not automatically a rename blocker. PID identity must be anchored with creation FILETIME, and causal claims require stronger evidence than presence in the query.

## Consequences

- We can cover the directory-only observability gap during investigation without making a reserved system interface part of the product contract.
- Future Windows versions/filesystems may return unsupported or different behavior; diagnostics must fail closed rather than affect normal engine removal.
- A passing calibration establishes usefulness on that tested environment only.
- If a supported public API later provides equivalent directory attribution, replacing this diagnostic mechanism requires a new review rather than silently promoting the current implementation.
