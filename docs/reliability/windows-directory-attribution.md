# Windows exact-directory user attribution — PR #35

Base: reviewed main after #32 (`814773fc2a6a9aaaa2fa025c19da19624c3e8270`). This is a diagnostics-only follow-up for Issue #27's intermittent Windows removal OS5. It does not include PR #34 startup changes or PR #33 distribution changes.

## Problem

Existing failure-time capture can identify users of selected regular files with Restart Manager and can refine those observations down to single files. That still leaves a blind spot: a process may hold the **version directory itself** (or another directory handle) while no regular-file query yields a witness. Historical and current OS5 failures therefore cannot be cleared by an empty file-user list.

PR #35 restores the previously preserved exact-directory experiment into a dedicated branch and requires fresh real-Windows calibration before treating it as useful evidence.

## Diagnostic mechanism

`tools/windows_directory_users.py` opens only the requested existing non-reparse directory with metadata access, all share flags and `OPEN_EXISTING`. It never requests delete disposition, never modifies ACLs/privileges, never duplicates another process's handle, and never unlocks or terminates a resource user.

The worker calls `NtQueryInformationFile` with `FileProcessIdsUsingFileInformation` (value 47). Microsoft documents this information class as **reserved for system use**, so YuTool treats it strictly as investigation tooling rather than a supported public API or runtime dependency.

The result is bounded to 128 PIDs and validates returned length, native pointer alignment and PID range. Unsupported, denied, truncated or malformed results remain explicit non-success observations. There is no retry that turns a changing or failed native snapshot into an apparent empty-success result.

For every PID from the first query, the collector opens only query/synchronization rights and holds that process handle across a second directory query. A user is identity-confirmed only when:

- the PID appears in the confirmation query;
- process creation FILETIME is available from the retained handle; and
- a zero-time wait shows that exact retained process is still live.

The collector process excludes its own PID. Correlation with Yu's lifecycle trace requires both PID and creation FILETIME.

## What this can and cannot establish

A confirmed result means that the exact process identity was observed using the exact opened directory object during the sampled interval. It still does **not** reveal the individual handle, share mode, kernel-filter state or causal relation to `MoveFileEx`/rename failure.

Therefore every output keeps:

```text
complete_handle_inventory = false
rename_blocker_proven = false
root_cause_fixed = false
```

A process may appear in the directory-user query while allowing delete sharing; such a process is a directory user but not necessarily a rename blocker. An empty result cannot prove absence of short-lived users, unsupported filesystem/filter behavior or a kernel-only cause.

## Failure-time integration

The existing bounded `collect_windows_occupancy.py` worker now performs the exact-directory query immediately after matching Yu-owned traces and **before** slower process enumeration / file selection / Restart Manager queries. This maximizes the chance of retaining a short-lived directory-only user without changing the original failed CLI result.

New snapshot files:

```text
02a-directory-users.json
02b-directory-correlation.json
```

All later existing stages remain unchanged. The same 10-second outer collector supervisor applies; if the native query hangs, only the collector subprocess can be terminated by the harness and partial prior evidence remains.

## Required real-Windows calibration

Three known-holder controls must run on Windows:

1. **Non-delete-sharing exact-directory holder** — the holder PID + creation time must be confirmed; the query must not unlock it; rename stays denied until only the test's own handle is released; afterwards the holder disappears and rename succeeds.
2. **Delete-sharing directory holder** — the same holder must still be observed while rename succeeds, proving that directory-user observation is not equivalent to rename-blocker proof.
3. **Sibling directory isolation** — holding `version-other` must not attribute that process to `version`.

The workflow retains bounded calibration receipts under `target/windows-directory-calibration/`.

## Scope boundary

No quarantine retry change, no extra delay before remove, no test serialization, no external process termination, no ACL/elevation change and no production-runtime dependency is introduced. Passing calibration does not retroactively attribute prior OS5 failures and does not authorize public v0.1 release.

Primary references:

- Microsoft `FILE_INFORMATION_CLASS`: `FileProcessIdsUsingFileInformation` is value 47 and reserved for system use.
- Microsoft `NtQueryInformationFile`: the returned byte count is provided through `IO_STATUS_BLOCK.Information`; unsupported classes/filesystems can fail and must remain explicit evidence gaps.

See [acceptance](../testing/pr35-windows-directory-attribution.md), [ADR 0013](../decisions/0013-windows-directory-query-diagnostic-only.md), and the earlier [occupancy capture](windows-occupancy-capture.md).
