# PR #31 — Bounded process and I/O acceptance

This is a main-derived runtime increment, not #29 startup integration, OS5 causation closure or public v0.1 authorization. Record exact feature SHA, hosted test-merge SHA, CLI/package hashes and completed suites separately. Never reuse #30 diagnostics success as proof for this runtime change.

## Fixed checks

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 -B -m unittest discover -s tools -p 'test_*.py' -v
cargo test --locked -p yu-runtime-psd --lib process::bounded_tests -- --nocapture
```

All8 bounded tests must execute on Windows, Linux and macOS. Empty reads must be WouldBlock, not false EOF. Fill the actual pipe to WouldBlock rather than assuming a capacity. Retained-peer read/write cases keep a test-owned handle outside the Job open throughout return, preserve the1-second timeout and require elapsed<4 seconds. No fixture holder may be killed to satisfy the assertion. Real-child observation must time out while it is still live, then explicit cleanup must confirm exit. The second resource shares the first resource's expired deadline.

Keep #28's execution/cleanup matrix and no-publication failure tests; document the worker-panic to cleanup-fault seam adaptation. The runtime must contain no production blocking wait/join, detached worker, forgotten buffer or global fault switch. Preserve selected-engine metadata, existing-output/source safety, output limits, explicit installation/activation and no engine fallback.

## Real programs

Run the unchanged Managed Package matrix on all3 platforms against both debug CLI and extracted release candidates, with real private Node/ag-psd, read operations, independent decoded PNG fingerprints and ten lifecycle cycles per suite. Run the independent Windows diagnostics' original3 passes without rerunning a failed case into green. Its focused bounded-transport step additionally runs the8 new tests, not zero tests with exit0. Retain raw failure artifacts and distinguish prescribed OS32 controls from spontaneous failures.

A current-head local Mac candidate must be built from clean committed source and tested after extraction. Report reused unchanged engine archives as reused, not rebuilt. Verify input/source identity before and after execution.

## Trace and remaining risks

Cleanup-end must show0 workers, closed endpoints, confirmed direct exit and Windows Job emptiness for successful invocations. A failed confirmation cannot be labeled successful completion. Historical trace layouts/counts must not be rewritten. The two-second cleanup poll budget is not a hard real-time guarantee over OS/filesystem/process creation/decoder calls. No new cancellation-token or Ctrl+C API is promised.

Keep main and #29 refs, experiment stash and M3 artifacts protected during development. No Release/tag, signature or candidate promotion. PR #29 and issue #27 remain separately tracked even if this change passes.
