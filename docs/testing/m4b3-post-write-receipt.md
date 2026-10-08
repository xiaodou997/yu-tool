# M4b-3: post-write verification and structured receipt

Base: `develop@b5dccf94f89ff5b568b2cd6c6079023fd2894daf`.
Feature branch: `feature/m4b3-output-verification-receipt`.
Frozen v0.1 `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af` stays unchanged.

## Contract

- Preserve the M4b-2 no-clobber / guarded replacement behavior.
- Check staging's actual decoded format and dimensions before publishing.
- After publish, reopen the output, check format/dimensions/length, compute
  SHA-256 and require published bytes to match the staged verification.
- Add `result.output_receipt` with `status` (`verified` or `planned`),
  optional `previous_sha256`, and on success `verified_output` with
  `sha256`, `bytes`, `width`, `height`, `format`.
- Dry-run has no `verified_output` and writes nothing.
- Pre-publication verification failure leaves the destination untouched.
  After-publication failure returns `VERIFICATION_FAILED` (exit 1) and
  warns that the output may already have been published. Never invent a
  success receipt or silently roll back.

## Local acceptance

- [x] `cargo fmt --all -- --check` (local macOS)
- [x] `cargo check --workspace --all-targets --locked` (local macOS)
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings` (local macOS)
- [x] `cargo test --workspace --locked` (local macOS; 174 passed, 0 failed)
- [x] `git diff --check` (local macOS)
- [x] All four mutations' receipts match actual published file SHA-256,
      decoded format, dimensions and filesystem byte length.
- [x] New outputs omit prior SHA; replacement includes validated old digest.
- [x] Dry-run returns planned without invented output hashes.
- [x] Invalid/corrupt bytes, wrong format and dimensions fail verification.
- [x] Injected after-publication corruption reports failure, not success.
- [x] Existing conflict protections and staging cleanup continue passing.

With GitHub Actions quota unavailable, use local tests and do not claim
cross-platform Windows/Linux acceptance. Do not create a PR/Actions run,
tag, public release, or change the v0.1 technical RC.

## Limits

Receipts describe a point-in-time check. Non-cooperating external writers may
change a file later. No filesystem compare-and-swap, power-loss durability,
or rollback after a failed post-publication verification is guaranteed.
