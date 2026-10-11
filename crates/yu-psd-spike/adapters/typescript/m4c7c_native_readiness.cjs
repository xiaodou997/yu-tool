"use strict";

// M4c-7c native Photoshop readiness: diagnostic only, never starts, clicks,
// dismisses dialogs, quits, restarts or saves user documents.
// A running process is NOT sufficient evidence that AppleScript is usable.

const { spawnSync } = require("node:child_process");

const APPLICATION_PATH =
  "/Applications/Adobe Photoshop 2026/Adobe Photoshop 2026.app/Contents/MacOS/Adobe Photoshop 2026";

function classifyPhotoshopReadiness(options = {}) {
  const run = options.run || spawnSync;
  const platform = options.platform || process.platform;
  const timeout = options.timeoutMs ?? 8000;
  if (platform !== "darwin") return { status: "unsupported_platform", ready: false };
  if (!Number.isInteger(timeout) || timeout < 500 || timeout > 15000) {
    throw Error("invalid bounded native timeout");
  }
  const processResult = run("/usr/bin/pgrep", ["-f", APPLICATION_PATH], {
    encoding: "utf8", timeout: 3000, maxBuffer: 32768,
  });
  if (processResult.error || processResult.status !== 0 ||
      !/^\d+(?:\s+\d+)*\s*$/.test(processResult.stdout || "")) {
    return { status: "photoshop_not_running", ready: false };
  }
  const query = run("/usr/bin/osascript",
    ["-e", 'tell application id "com.adobe.Photoshop" to count documents'], {
      encoding: "utf8", timeout, maxBuffer: 32768,
    });
  if (query.error?.code === "ETIMEDOUT") {
    return { status: "apple_events_unresponsive", ready: false };
  }
  const detail = String(query.stderr || query.error?.message || "");
  if (query.error || query.status !== 0) {
    if (/(-1743|-1728|not authorized|不允许|没有权限|permission|not permitted)/i.test(detail)) {
      return { status: "native_permission_denied", ready: false };
    }
    return { status: "apple_events_error", ready: false };
  }
  const answer = (query.stdout || "").trim();
  if (!/^\d+$/.test(answer)) {
    return { status: "unexpected_application_response", ready: false };
  }
  const openDocuments = Number(answer);
  if (!Number.isSafeInteger(openDocuments)) {
    return { status: "unexpected_application_response", ready: false };
  }
  if (openDocuments !== 0) {
    return { status: "existing_documents_open", ready: false, open_documents: openDocuments };
  }
  return { status: "ready_empty_workspace", ready: true, open_documents: 0 };
}

module.exports = { classifyPhotoshopReadiness };
