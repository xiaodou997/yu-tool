# PR #26 — Windows lifecycle diagnostic acceptance

Run on the exact committed PR head; record actual checkout SHA separately for GitHub test-merge candidates. This checklist is not a prior success receipt.

## Ordinary gate

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
python -B -m unittest discover -s tools -p 'test_*.py' -v
```

`cargo test --locked -p yu-cli --test psd transport_lifecycle -- --nocapture` must execute four cases on Unix and five on Windows. Check EOF/process-exit phases, nonzero partial-response byte counts, unchanged source and full removal. All old timeout, protocol, pixel and archive assertions remain enabled.

## Windows extracted-candidate investigation

Use a clean checkout and the existing private-engine/candidate builders. The standalone Windows Lifecycle Diagnostics workflow performs the complete build/probe sequence. To replay with already verified local test inputs:

```powershell
python -B tools/run_lifecycle_probe.py --cli target/windows-lifecycle-candidate/smoke/bin/yu.exe --manifest target/windows-lifecycle-engine/manifest-windows-x86_64.json --archive target/windows-lifecycle-engine/yu-engine-ag-psd-31.0.2-node22.23.3.yu2-windows-x86_64.zip --repetitions 3 --output-dir target/windows-lifecycle-report
```

Use a new output directory for each invocation; prior evidence must never be overwritten. The report must contain six completed steps, input hashes/unchanged status and30 completed real lifecycle cycles across the three managed-suite logs. The probe rejects an exit0 with zero/insufficient executed-test evidence. Original parallel execution remains active.

On failure preserve `report.json` and the failing log. Separate setup/outer timeout, direct process exit, input completion, output EOF and removal-quarantine failures. Never infer the owner of a Windows handle from OS5 alone. Do not relabel a passing rerun as a repair.

## Review boundary

Confirm that candidate `public_release_ready` stays false and no release/tag/hosting/license/signing claim is introduced. Confirm this change does not modify native Job assignment, process termination/cleanup, quarantine retry budget, package payload, frozen M3 receipts or independent pixel expectations. Native-control hardening remains unimplemented follow-up, with fresh reproduction and acceptance required.
