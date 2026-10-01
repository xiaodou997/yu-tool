# PR #34 — Windows startup deadline acceptance

Originally developed from #32 and rebased for final acceptance onto `main@f8b461c14c1c71ac76117c4eb2aca43b570db0dd` after #37. Keep PR #33 distribution work separate. Preserve Issue #27 and all historical OS5/timeout evidence.

## Local gate

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 -B -m unittest discover -s tools -p 'test_*.py' -v
```

The controlled startup-overrun unit test must use the original operation clock, let a test-only startup seam return after that deadline, and verify `phase=startup` is returned before the transport loop. It must not add a public fault flag, increase the timeout or depend on detached work.

## Native Windows gate

```sh
cargo test --locked -p yu-runtime-psd --lib process::windows_spawn::tests -- --nocapture
cargo test --locked -p yu-runtime-psd --lib process::bounded_tests -- --nocapture
cargo test --locked -p yu-cli --test psd transport_lifecycle -- --nocapture
```

Require the existing creation-time Job/handle-list, early-descendant, overlapping-invocation, 64 KiB input, query-denial, retained-member and bounded-I/O controls to stay green. At least one real startup control must observe the ordered inner startup phases. With `YU_WINDOWS_LIFECYCLE_TRACE_DIR` enabled, a production invocation's `started` trace must contain all nine phase names including `job_create` and `create_process`.

A startup that returns after the operation deadline must not proceed into protocol exchange. Cleanup must continue using the existing two-second shared deadline and retain the existing no-publication/no-clobber behavior.

## Real package gate

Run the existing three-platform Managed Package workflow and Windows Lifecycle Diagnostics without changing default parallelism, the 30-second engine timeout, cleanup budget or failed-case behavior. A failure is retained as evidence and is not replaced by a rerun. In particular, keep PR #33 Package28's 66.6-second startup-interval failure as historical evidence even if this branch does not reproduce it.

If another slow startup occurs, report the exact source/candidate identity and the startup stage durations. A slow `create_process` observation still does not by itself establish why Windows delayed that call.

## Delivery boundary

Successful tests may justify merging this runtime semantic/diagnostic improvement; they do not close Issue #27, certify a hard wall-clock startup bound, select a project license, accept minimum OS/signing, or authorize v0.1 publication.
