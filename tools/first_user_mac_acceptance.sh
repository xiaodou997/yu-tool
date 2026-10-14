#!/bin/bash
# YuTool v0.1.0-dev First User Acceptance — macOS ARM64.
# No sudo, no xattr removal, no Gatekeeper modification, no user PSDs.
# Use: bash First-User-Test.command [--verify-only|--run]
set -u
set -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
OUTER="yu-v0.1.0-dev.5526dc4-macos-arm64.zip"
OUTER_SHA="5897360ddbc770e45232b2bbbf569cb585a88d38547105a6ae872b6e72c8e44a"
CLI="yu-0.1.0-dev.5526dc4465c4-aarch64-apple-darwin.zip"
CLI_SHA="388fb2f1f60cbca3166d6b14046ad06595813f45388e0c482d99defbc611bcfc"
ENGINE="yu-engine-ag-psd-31.0.2-node22.23.3.yu2-macos-aarch64.zip"
ENGINE_SHA="cd1dc8caa0c8294114b39d8d5573e6d8647cd7aea09e094b073ad27afd7b406c"
BIN_SHA="22efb3cc1a47e8ccc052b14405a440457aeb3a9bdf5bb2b9144b46da03ba6901"
PSD_SHA="406ddf7cbf5a1065b992b9c0d2a59be6290a371b90f23b9a04c0e6bbc94beb0c"
PNG_SHA="7f2538a6e81af23e034f8ca2efc2cb5f1d7d709f502fa30bdccfebe774e4ecab"
SOURCE="5526dc4465c4e79ad7f2757d06b3b1693642b8af"
VERSION="31.0.2+node22.23.3.yu2"

MODE="${1:---run}"
case "$MODE" in
  --run|--verify-only) ;;
  *) echo "用法：bash First-User-Test.command [--run|--verify-only]" >&2; exit 2 ;;
esac
if [ "$(/usr/bin/uname -s)" != "Darwin" ] || [ "$(/usr/bin/uname -m)" != "arm64" ]; then
  echo "REFUSED: 本脚本仅允许 macOS Apple Silicon。" >&2; exit 2
fi
if [ ! -f "$ROOT/$OUTER" ] || [ -L "$ROOT/$OUTER" ]; then
  echo "REFUSED: 未找到原始内测 ZIP 或文件是符号链接。" >&2; exit 2
fi
if [ ! -f "$ROOT/test-fixtures/2layers.psd" ] || [ -L "$ROOT/test-fixtures/2layers.psd" ]; then
  echo "REFUSED: 测试 PSD 样本缺失或不安全。" >&2; exit 2
fi
if [ ! -f "$ROOT/test-fixtures/sample.png" ] || [ -L "$ROOT/test-fixtures/sample.png" ]; then
  echo "REFUSED: 测试 PNG 样本缺失或不安全。" >&2; exit 2
fi
sha() { /usr/bin/shasum -a 256 "$1" | /usr/bin/awk '{print $1}'; }
check_hash() {
  local actual
  actual="$(sha "$1")"
  if [ "$actual" != "$2" ]; then
    echo "REFUSED: SHA-256 不一致 $(/usr/bin/basename "$1")" >&2
    return 1
  fi
}
check_hash "$ROOT/$OUTER" "$OUTER_SHA" || exit 2
check_hash "$ROOT/test-fixtures/2layers.psd" "$PSD_SHA" || exit 2
check_hash "$ROOT/test-fixtures/sample.png" "$PNG_SHA" || exit 2

TMP_BASE="${TMPDIR:-/tmp}"
if [ ! -d "$TMP_BASE" ]; then echo "REFUSED: 缺少临时目录" >&2; exit 2; fi
RUN="$(/usr/bin/mktemp -d "${TMP_BASE%/}/yu-first-user-XXXXXXXX")" || exit 2
/bin/chmod 700 "$RUN"
/bin/mkdir -p "$RUN/outer" "$RUN/app" "$RUN/logs" "$RUN/home" "$RUN/temp" "$RUN/user-data"
REPORT="$RUN/report.tsv"
/usr/bin/printf 'step\tstatus\texit_code\n' > "$REPORT"
mark() { /usr/bin/printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$REPORT"; }
echo "本次独立测试日志：$RUN"
echo "来源 SHA：$SOURCE"

if /usr/bin/xattr -p com.apple.quarantine "$ROOT/$OUTER" > "$RUN/logs/original-zip-quarantine.txt" 2>&1; then
  mark ZIP_QUARANTINE PRESENT 0
else
  mark ZIP_QUARANTINE ABSENT_OR_NOT_READABLE 0
fi
/usr/sbin/spctl --status > "$RUN/logs/gatekeeper-status.txt" 2>&1 || true
/usr/bin/sw_vers -productVersion > "$RUN/logs/macos-version.txt" 2>&1 || true
/usr/sbin/sysctl -n hw.model > "$RUN/logs/mac-model.txt" 2>&1 || true

# The pinned outer ZIP was audited against these exact member hashes.
# Never extract an unrecognized or changed archive.
if ! /usr/bin/ditto -x -k "$ROOT/$OUTER" "$RUN/outer" > "$RUN/logs/extract-outer.txt" 2>&1; then
  mark EXTRACT_OUTER FAIL 1; echo "外层 ZIP 解压失败，查看日志 $RUN" >&2; exit 1
fi
mark EXTRACT_OUTER PASS 0
CANDIDATE="$RUN/outer/candidate/$CLI"
MANIFEST="$RUN/outer/engine/manifest.json"
ENGINE_PATH="$RUN/outer/engine/$ENGINE"
if [ ! -f "$CANDIDATE" ] || [ -L "$CANDIDATE" ] ||
   [ ! -f "$ENGINE_PATH" ] || [ -L "$ENGINE_PATH" ] ||
   [ ! -f "$MANIFEST" ] || [ -L "$MANIFEST" ]; then
  mark INNER_ARCHIVES FAIL 1; echo "缺少内层原始归档。" >&2; exit 1
fi
if ! check_hash "$CANDIDATE" "$CLI_SHA" || ! check_hash "$ENGINE_PATH" "$ENGINE_SHA"; then
  mark INNER_ARCHIVES FAIL 1; exit 1
fi
mark INNER_ARCHIVES PASS 0
if ! /usr/bin/ditto -x -k "$CANDIDATE" "$RUN/app" > "$RUN/logs/extract-cli.txt" 2>&1; then
  mark EXTRACT_CLI FAIL 1; exit 1
fi
BIN="$RUN/app/bin/yu"
if [ ! -f "$BIN" ] || [ -L "$BIN" ] || ! check_hash "$BIN" "$BIN_SHA"; then
  mark BINARY_SHA FAIL 1; exit 1
fi
mark BINARY_SHA PASS 0

if /usr/bin/xattr -p com.apple.quarantine "$BIN" > "$RUN/logs/extracted-binary-quarantine.txt" 2>&1; then
  mark EXTRACTED_QUARANTINE PRESENT 0
else
  mark EXTRACTED_QUARANTINE ABSENT_OR_NOT_READABLE 0
fi
/usr/bin/codesign --verify --strict "$BIN" > "$RUN/logs/codesign.txt" 2>&1 || true

if [ "$MODE" = "--verify-only" ]; then
  echo "文件及内层 CLI 的 SHA-256 校验通过；没有执行程序。"
  echo "结果：$REPORT"
  exit 0
fi

# Empty environment: do not read or modify the operator's regular
# HOME, existing YuTool engines, shell profiles, or developer tools.
export LC_ALL=C
failure=0
step() {
  local name="$1"; local expect="$2"; shift 2
  /usr/bin/env -i HOME="$RUN/home" USERPROFILE="$RUN/home" \
    YU_DATA_HOME="$RUN/user-data" TMPDIR="$RUN/temp" \
    XDG_CONFIG_HOME="$RUN/home/config" XDG_DATA_HOME="$RUN/home/data" \
    PATH="/usr/bin:/bin" "$BIN" "$@" \
    > "$RUN/logs/$name.stdout" 2> "$RUN/logs/$name.stderr"
  local rc=$?
  if [ "$rc" -eq "$expect" ]; then
    mark "$name" PASS "$rc"
  else
    mark "$name" FAIL "$rc"
    echo "FAILED $name: 预期 $expect，实际 $rc（参见 $RUN/logs）" >&2
    failure=1
    return 1
  fi
}

step VERSION 0 --version || exit 1
if ! /usr/bin/grep -qx "yu 0.1.0" "$RUN/logs/VERSION.stdout"; then
  mark VERSION_ID FAIL 1; echo "可执行文件版本不符合冻结的 v0.1.0 源码。" >&2; exit 1
fi
step DOCTOR 0 doctor --json || exit 1
step CAPABILITIES 0 capabilities --json || exit 1
step IMAGE_INFO 0 image info "$ROOT/test-fixtures/sample.png" --json || exit 1
step IMAGE_RESIZE 0 image resize "$ROOT/test-fixtures/sample.png" --width 1 -o "$RUN/resized.png" --json || exit 1
step RESIZED_IMAGE_INFO 0 image info "$RUN/resized.png" --json || exit 1

# This PSD fixture is included in the kit. Nothing is read from the
# operator's documents, and PNG export always targets this new run.
step PSD_BEFORE_INSTALL 3 psd inspect "$ROOT/test-fixtures/2layers.psd" --json || exit 1
step ENGINE_INSTALL 0 engine install --manifest "$MANIFEST" --archive "$ENGINE_PATH" --json || exit 1
step PSD_BEFORE_ACTIVATE 3 psd inspect "$ROOT/test-fixtures/2layers.psd" --json || exit 1
step ENGINE_ACTIVATE 0 engine activate ag-psd "$VERSION" --json || exit 1
step PSD_INSPECT 0 psd inspect "$ROOT/test-fixtures/2layers.psd" --json || exit 1
step PSD_TREE 0 psd tree "$ROOT/test-fixtures/2layers.psd" --json || exit 1
step LAYER_LIST 0 psd layer list "$ROOT/test-fixtures/2layers.psd" --json || exit 1
step LAYER_EXPORT 0 psd layer export "$ROOT/test-fixtures/2layers.psd" --id L0001 -o "$RUN/test-layer.png" --json || exit 1
if [ ! -s "$RUN/test-layer.png" ]; then
  mark EXPORTED_FILE FAIL 1; echo "图层未能正常导出。" >&2; exit 1
fi
step ENGINE_DEACTIVATE 0 engine deactivate ag-psd --json || exit 1
step ENGINE_REMOVE 0 engine remove ag-psd "$VERSION" --json || exit 1

if ! check_hash "$ROOT/$OUTER" "$OUTER_SHA" ||
   ! check_hash "$ROOT/test-fixtures/2layers.psd" "$PSD_SHA" ||
   ! check_hash "$ROOT/test-fixtures/sample.png" "$PNG_SHA"; then
  mark ORIGINAL_INPUTS_UNCHANGED FAIL 1; exit 1
fi
mark ORIGINAL_INPUTS_UNCHANGED PASS 0
echo "当前设备隔离功能检查通过。注意：此结果不等于 MacBook Air 首次下载/Gatekeeper 验收通过。"
echo "完整报告：$REPORT"
exit "$failure"
