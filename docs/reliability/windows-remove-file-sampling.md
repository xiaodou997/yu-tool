# Windows in-flight regular-file attribution — PR #37

Base: `main@5a13e7e249e5b88b6a1313bcff7ea1acaabd8012` after PR #36.

PR #36 reproduced Issue #27's intermittent Windows engine-removal OS5 while 81 exact-directory samples over about two seconds observed no directory user. The immediate post-failure exact-directory query was also empty and Yu-owned Job cleanup was complete. A later Restart Manager snapshot did, however, identity-confirm a transient process as a user of at least one registered regular file, but that relationship disappeared before the post-failure partitioner could retain a singleton file witness.

This increment moves the existing bounded Restart Manager observation **into the remove window**. It remains test/diagnostic infrastructure only.

## Sampling model

When `YU_TEST_REMOVE_FILE_SAMPLING_DIR` and the existing forensic Python are explicitly enabled, the test harness starts a second owned sampler before `yu engine remove`.

The sampler:

1. selects at most the existing bounded 64 regular files from the exact version tree without following reparse points;
2. opens a fresh Restart Manager session for the selected files and identity-confirms every reported process by PID + creation FILETIME;
3. if a verified identity is observed, immediately runs the existing bounded partitioning algorithm to seek a positive singleton witness;
4. repeats while remove is in flight, nominally every 50 ms, with a five-second / 128-sample outer bound;
5. stops on remove completion, target disappearance, the sampling bound, or explicit harness stop;
6. never shuts down, unlocks, suspends, kills or changes privileges/ACL/share modes of any observed process.

Each positive singleton means only:

> the same identity-confirmed process was reported by Restart Manager for this one registered file at that observation.

It does **not** reveal the process handle's share flags and does not prove that the process caused a directory rename denial.

## Observer effect and limits

Restart Manager sessions and file registration are themselves observable work and can perturb timing. Native Restart Manager calls are not individually interruptible; the Rust harness may terminate only its own sampler process if that owned diagnostic subprocess fails to stop within its bounded reap window.

Coverage is limited to the selected regular files. Directory-only handles, unselected files, kernel/filter-driver activity and users that appear only between samples remain outside the evidence.

Every report preserves:

```text
rename_blocker_proven = false
root_cause_fixed = false
public_release_ready = false
```

## Relation to the PR #36 evidence

The leading reason for this increment is the real PR #36 reproduction:

- remove: OS error 5 after 76 attempts / 2001 ms;
- exact-directory samples: 81 successful queries, zero identities;
- post-failure exact-directory snapshot: zero identities;
- Yu-owned Job cleanup: complete, zero retained members;
- post-failure regular-file Restart Manager snapshot: one identity-confirmed transient process;
- post-failure partitioning: relationship persisted through a two-file batch but vanished before either singleton could be retained.

PR #37 attempts to retain that file relationship earlier, while the remove failure is still occurring. It does not assume the previously observed process is the root cause.

See [PR #37 acceptance](../testing/pr37-windows-remove-file-sampling.md).
