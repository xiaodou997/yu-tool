# PR #26 follow-up — Windows occupancy capture

This slice adds evidence collection, **not a root-cause fix or release approval**. Historical OS 5 and 30-second timeout records stay immutable. No native process-creation, Job-assignment, termination ordering, quarantine-retry or public timeout policy is changed.

## Two correlated evidence streams

The Windows-only `YU_WINDOWS_LIFECYCLE_TRACE_DIR` opt-in accepts an existing absolute directory. The runtime records separate, exclusively created JSON files at `started`, `cleanup_begin` and `cleanup_end`. Each includes the direct-child PID and creation FILETIME, version directory, invocation identity and the private Job's basic accounting snapshot. Cleanup-end records whether the existing direct-child wait succeeded and how many worker joins returned. The original process-control calls and their order remain unchanged. With the opt-in absent, these extra filesystem and Job-query calls are not made. The trace is best effort: absent or partial files are evidence gaps, not successful cleanup.

The independent probe enables that trace and a **test-harness-only** post-failure hook. On a failed public `engine remove`, the hook preserves the original exit/error and invokes a Python collector before assertion unwinding can delete the isolated engine directory. It does not rerun the failed command or modify its JSON. This is **after-command/pre-teardown sampling**, not an atomic snapshot at the instant the kernel rejected rename. The observer may affect timing; the original uninstrumented package workflow remains separate.

`tools/collect_windows_occupancy.py` supervises an owned worker with a ten-second native-query budget and retains each completed stage even if later API calls hang. An outer filesystem stall is not covered by a hard real-time guarantee. The query worker does not start any children. Only that owned worker may be terminated on collector timeout; resource users are never terminated.

## Snapshot contents

- Original failed CLI error/exit and command-finished timestamp, retained separately from query status.
- Directory attributes and attempts to open the source/parent with DELETE access, all sharing flags and OPEN_EXISTING. Handles are immediately closed; no delete disposition or delete-on-close flag is used. These probes do not prove effective ACL correctness or identify an owner.
- Matching owned-process trace records, preserving PID **and creation time** to avoid PID-reuse misattribution.
- At most 4096 process IDs queried with limited-information rights. Only processes whose executable path lies within the tested version are retained; access failures and truncation are counted. No unrelated command lines, environment or document contents are collected.
- A bounded scan of at most 512 directory entries, 64 regular files and depth five inside this one version. Symlinks/junctions are not followed during traversal. As with other same-user diagnostics, concurrent malicious replacement of ancestor directories is not sandboxed. The selected path list and omissions are retained.
- One Restart Manager resource registration and bounded query (up to 128 process records), with API return codes and process creation times. A changing/oversized list is reported as incomplete, not retried into apparent completeness.
- Correlation between reported file users and recorded direct engine children by PID + FILETIME. An unmatched process is **not automatically an external blocker**; unrecorded descendants and races remain possible.

Restart Manager cannot query directory paths as file resources; `RmGetList` documents ERROR_ACCESS_DENIED for a registered directory. Its result identifies applications using the selected **files**, not every directory-only handle, kernel filter or permission restriction. A file user is not necessarily the process preventing a parent-directory rename. Empty results must never be described as proof of no occupancy. Registration creates Restart Manager session bookkeeping but does not change resource owners, sharing modes or ACLs. No RmShutdown/RmRestart or unlock/elevation operation is used.

Job accounting is observed while the runtime still owns process handles. A nonzero ActiveProcesses count is not, by itself, proof of a leaked running process; a zero count is not proof that every external handle/reference is closed. The trace does not enforce whole-Job completion or close the existing start-before-assignment gap.

## Validation

`python -B -m unittest discover -s tools -p 'test_windows_occupancy.py' -v` includes portable bounds, path/correlation, missing-cleanup and timeout-preservation checks. Two additional Windows tests use only self-created handles: a real file holder must be discoverable from a separate collector process without being unlocked, and a directory sharing conflict must remain a conflict until its own handle is released.

The independent Windows workflow runs those tests before building the candidate. Its existing five native cases and real PSD/lifecycle suites then run against the extracted candidate. The quiet-descendant case requires complete matching trace stages when tracing is enabled. Expected directory-denial controls also create occupancy snapshots, which must be distinguished from spontaneous real-engine failures by their test roots and original errors. All original deadlines, parallelism, first-failure stopping and release-blocked flags remain in place. A passing diagnostic run cannot supersede a historical failure.

## Evidence layout

```text
windows-lifecycle-report/
  report.json
  01-transport.log
  01-managed.log
  owned-process-traces/*-{started,cleanup_begin,cleanup_end}.json
  occupancy/occupancy-<test-pid>-<time>-<sequence>/
    context.json
    snapshot/
      trigger.json
      01-path-observation.json
      02-owned-traces.json
      03-version-processes.json
      04-selection.json
      05-resource-users.json
      06-correlation.json
      07-file-attribution-events.jsonl
      08-file-attribution.json
      collector.log
      report.json
```

The count of captures alone is not success evidence; inspect per-capture status, stage coverage and timestamps. The workflow retains this directory even when a test fails. No public candidate promotion occurs.

## Bounded single-file follow-up (PR #30)

The initial batch query above is retained unchanged. A diagnostics-only refinement now seeks at most one single-file witness for each verified batch identity, using fresh sessions, at most16 extra queries and a3-second scheduling budget inside the existing10-second collector supervisor. Starts and completions are journaled separately; incomplete/missing summaries never imply no occupancy. This does not identify directory-only handles, prove share-mode conflict or retroactively attribute the #29 failure. See [full scope and calibration](windows-resource-attribution.md) and [acceptance](../testing/pr30-windows-resource-attribution.md).

## Primary references

- https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/nf-restartmanager-rmgetlist
- https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/nf-restartmanager-rmregisterresources
- https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/ns-restartmanager-rm_unique_process
- https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-queryinformationjobobject
- https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_accounting_information
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew
