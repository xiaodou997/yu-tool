# Windows single-file resource attribution — PR #30

Base: `62b022911dcca5bd06f04a6ca76d80202195520b` (main after #28). This diagnostics-only branch is independent of unmerged #29. It does not alter Rust runtime code, startup ownership, process termination, pipe handling, quarantine retries, package payloads or dependencies. Historical failures and the saved directory-query experiment remain unchanged.

## Why this increment exists

The final #29 Windows Diagnostics7 run (`36409219792`, attempt 1) failed at round 2, cycle 10, during engine quarantine: OS5, 76 attempts, 2001 ms. Nineteen full lifecycle cycles completed; round 3 did not run. The resource query identified PID5488 and a matching creation FILETIME134350644581518749, with image basename `provjobd.exe3031129891`. Its result covers at least one of the registered regular files, NOT a specified file or the handle causing the directory rename failure. Neither the name nor the observation identifies a vendor or proves external causation.

The original artifact10963158976 (407886 bytes) has SHA256 `11fde0ca7f2e4eebc0c8a8acd02fabeb8353d2e6e0379fcdcb216b1c2fc53bfb`. Do not modify that artifact or fill in a historical file attribution using later queries. The historical process may no longer exist; new observations use new PID/creation-time pairs.

## Separate acceptance decisions

#29's exact feature `c084c9edf6ece6af9f2f390ff6dfb3a21b744171` passed the focused creation-time membership/permission/argument tests and CI114/Managed Package23. Its complete independent Windows Diagnostics7 gate failed. That is evidence for the startup contract but not an all-green lifecycle acceptance or permission to merge. Keep #29's draft state and issue #27 open. This PR does not retarget, waive, rerun or relabel that failed gate.

## Query contract

After the existing batch query and correlation are saved, `tools/windows_resource_attribution.py` refines only verified, still-live batch identities. Both the batch and a later query must match PID, process creation FILETIME and the independent process-time observation; reused PIDs, inaccessible identities and observed exited processes do not become witnesses.

A positive multi-file partition is subdivided until a positive **single-file** query is observed. Each query uses a fresh Restart Manager session via the existing `WindowsAPI.resource_users`; registrations never accumulate across partitions. Negative or changing snapshots do not imply that the complement has the original user. A singleton witness describes one sampled regular-file relationship, never a share-mode conflict, directory handle, causal rename blocker or complete process/handle inventory. Stop after finding one file witness per verified target, not after enumerating every open file.

This path is diagnostic-only because Restart Manager registration is relatively expensive. It does not perform a per-file sweep on every command: the usual batch remains first, only positive identities are refined, and queries are bounded.

Fixed bounds:

- The existing selection remains at most64 regular files /512 entries /depth5, all inside the failed test version. Recheck selected file and ancestor metadata before each query; no reparse traversal or payload reads. Concurrent malicious same-user path replacement is not sandboxed.
- At most16 verified baseline identities and16 additional native queries. Baseline truncation and omitted identities remain explicit.
- Stop scheduling after3 seconds. This is not a hard native-call deadline: the existing10-second collector worker supervisor is unchanged. A hung native call leaves its start record; the outer collector may terminate ONLY its own worker, never any resource user.
- Do not retry a failed query into apparent completeness. Session-end failure stops further registrations. Unsupported, malformed, truncated, unavailable and changing results remain unresolved.

## Evidence

Existing files01–06 are unchanged. The new files are:

```text
snapshot/
  07-file-attribution-events.jsonl
  08-file-attribution.json
```

The journal is exclusively created and flushed before and after each native call. Completed queries retain their actual resource list and timestamps. An interrupted query has a start but no completion; missing final summary is incomplete evidence, never zero users. The summary reports witnesses, unresolved identities, query/path failures, scheduling/selection limits and stop reason. `rename_blocker_proven`, `directory_handles_covered` and `root_cause_fixed` remain false.

`collected` in the outer report means the collection function returned, not that an identity was localized. Examine the summary's status and journal as well. No external command lines, environments, process memory or file payloads are read. No uninstaller retry, unlocker, RmShutdown/RmRestart, privilege/ACL change, scan exclusion or public-release promotion is included.

## Controlled calibration

Portable tests cover identity changes, missing metadata, partition/singleton distinction, negative complements, fixed budgets, session errors, interrupted journals, paths and reparse changes. Real Windows tests use only test-created holders:

1. Two processes holding two different files must be mapped to the correct individual files by a third collector process, without changing either held handle; rename stays denied until the test releases its own handles.
2. Sequential native queries for different files must not reuse accumulated registrations.
3. A file opened with delete-sharing can still be renamed while held; positive OR empty RM observations must not claim a proven blocker. This control does not promise that RM reports every permissively shared handle.

Native calibration is its own Windows job. The existing three-pass extracted-CLI diagnostic job remains separate, with first-failure stopping and the original timeout/retry/parallelism. Its failures must stay failures even if calibration passes. Native calibration snapshots are retained under `target/windows-resource-calibration/`; temporary fixture data is removed afterward. Final exact-commit results belong in the PR receipt, not in this document before they exist.

## Follow-up boundaries

Per-file evidence can identify a candidate resource, not retroactively determine the historical directory lock. If a new spontaneous failure occurs, correlate its batch, singleton query times, PID/FILETIME and own cleanup traces without substituting later successes for the original failure. Whole-Job completion, bounded process waiting and I/O cancellation remain separate runtime changes. No Release/tag is created by this work.

Primary documentation:
- https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/nf-restartmanager-rmgetlist
- https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/nf-restartmanager-rmregisterresources
- https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/ns-restartmanager-rm_unique_process
