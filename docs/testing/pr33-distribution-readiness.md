# PR #33 — Distribution readiness acceptance

## Scope and source fences

Final acceptance starts from `main@f0a3f052e52ce9ba89c59bbebbce813b3b84410e`
after #34/#37. Verify the feature SHA and each workflow's actual checkout/candidate
SHA separately. PR #29 remains closed as superseded; retain its historical failures.
Do not restore or delete stash `613868efec1f6e27f85dc8918efec64e5a981987`.
PR #33 must not regress the current runtime, engine-manager, Windows diagnostics,
`.yu2` payload, Cargo/npm locks or M3 evidence. No release, tag, legal license
selection or historical-risk closure.

## Portable gate

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 -B -m unittest discover -s tools -p 'test_*.py' -v
```

Require legacy v1 and new v2 archive round trips; deterministic same-input ZIPs;
rejection of altered/missing notices, wrong source identity, unexpected paths,
symlink entries and incorrect archive hashes before execution/extraction.
Check graph traversal excludes dev-only/unreachable packages, preserves nested
notice bytes and upstream expressions, rejects outside-package links/paths, and
records missing evidence without asserting approval. Test environment filtering
must drop developer credentials, Cargo paths and runtime tracing.

## Actual native build and isolated invocation

Requires Python 3.11+ and the pinned Rust build toolchain for BUILDING only.
For macOS ARM64 (use the matching target and engine input on Linux/Windows):

```sh
python3 -B tools/build_cli_candidate.py \
  --target aarch64-apple-darwin --output-dir target/pr33-candidate
python3 -B tools/accept_cli_candidate.py \
  --candidate-dir target/pr33-candidate \
  --manifest PATH/manifest-macos-aarch64.json \
  --engine-archive PATH/yu-engine-ag-psd-31.0.2-node22.23.3.yu2-macos-aarch64.zip \
  --fixture fixtures/psd/upstream/psd-tools/2layers.psd \
  --expected-source EXACT_CHECKOUT_SHA \
  --output-dir target/pr33-isolated-acceptance
```

Use new output directories per attempt; do not overwrite an old receipt. The
harness runs core image operations and explicit offline PSD installation through
removal without Cargo/npm/system Node in the tested child's environment. Check
all 20 expected command steps executed, input hashes unchanged, output conflict
returned OUTPUT_CONFLICT, removal_complete=true and no trace flag was inherited.
The optional engine inventory must include original Node/ag-psd/base64-js/pako
notice file hashes. Candidate build-info must bind notices/usage to the same target
and source. Legal review, clean VM, network isolation and public release remain
false/unaccepted even when tests pass. Record actual component counts per target,
not one platform's count presented as universal.

## Hosted gates and record location

Require exact-head standard CI, three-platform Managed Package workflow including
its original pixel fingerprints/repeated cycles AND isolated acceptance, plus the
unchanged Windows joint/lifecycle tests. First failures remain evidence; no
failed-case retries or test serialization changes. Distinguish fresh environment
from fresh operating system, and successful finite samples from closing issue #27.
Record workflow IDs, actual tests and limitations in the PR and repository docs.
No downloadable acceptance attachment is required for handoff.
