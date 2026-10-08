"use strict";

// M4c-2 research-only read-only preflight. NEVER writes PSDs and is not
// registered as a YuTool production CLI/protocol/Managed capability.
// Only byte-identical committed fixture identities can become research
// candidates; general PSD feature-allowlisting still requires a raw-block
// inventory and independent editor-level fidelity evidence.

const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");

const ROOT = path.resolve(__dirname, "../../../../");
const POLICY_PATH = path.join(ROOT, "docs/data/psd-safe-mutation-scope-m4c2-v1.json");
const ENGINE_PATH = path.join(ROOT, "packaging/ag-psd-engine/node_modules/ag-psd");
const REQUEST_FIELDS = [
  "contract_version", "operation", "input_path", "expected_source_sha256",
  "layer_id", "expected_old_name", "new_name", "output_path",
];

class PreflightError extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
  }
}

function reject(code, message) {
  throw new PreflightError(code, message);
}

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function requireString(request, field) {
  const value = request[field];
  if (typeof value !== "string" || !value.length) {
    reject("INVALID_ARGUMENT", `${field} must be a non-empty string`);
  }
  if (value.includes("\0")) reject("INVALID_ARGUMENT", `${field} cannot contain NUL`);
  return value;
}

function assertSha(value, field) {
  if (!/^[0-9a-fA-F]{64}$/.test(value)) {
    reject("INVALID_ARGUMENT", `${field} must be exactly 64 hexadecimal characters`);
  }
  return value.toLowerCase();
}

function canonicalId(number) {
  return `L${String(number).padStart(4, "0")}`;
}

function flattenedLayers(psd, limit) {
  const layers = [];
  function walk(children, depth) {
    if (depth > 32) reject("UNSUPPORTED_DOCUMENT", "document nesting exceeds scope");
    if (!Array.isArray(children)) return;
    for (const layer of children) {
      if (layers.length >= limit) reject("UNSUPPORTED_DOCUMENT", "document layer count exceeds scope");
      layers.push({
        id: canonicalId(layers.length + 1),
        name: layer.name,
        kind: Array.isArray(layer.children) ? "group" :
          layer.text ? "text" : layer.placedLayer ? "smart_object" :
            layer.vectorFill ? "shape" : "pixel",
        has_mask: !!layer.mask || !!layer.realMask || !!layer.vectorMask,
        has_effects: !!layer.effects
      });
      walk(layer.children, depth + 1);
    }
  }
  walk(psd.children, 1);
  return layers;
}

function validateDestination(inputPath, outputPath, format) {
  const input = path.resolve(inputPath);
  const output = path.resolve(outputPath);
  if (path.extname(output).toLowerCase() !== `.${format}`) {
    reject("INVALID_ARGUMENT", "output extension must match the original PSD/PSB format");
  }
  const parent = path.dirname(output);
  let parentInfo;
  try {
    parentInfo = fs.statSync(parent);
  } catch {
    reject("INVALID_INPUT", "output directory does not exist or cannot be inspected");
  }
  if (!parentInfo.isDirectory()) reject("INVALID_INPUT", "output parent is not a directory");
  try {
    fs.lstatSync(output);
    reject("OUTPUT_CONFLICT", "output path exists, including symlink or directory");
  } catch (error) {
    if (!(error && error.code === "ENOENT")) throw error;
  }
  const resolvedParent = fs.realpathSync(parent);
  const equivalentOutput = path.join(resolvedParent, path.basename(output));
  if (equivalentOutput === fs.realpathSync(input)) {
    reject("OUTPUT_CONFLICT", "source modification is forbidden; output must be distinct");
  }
  return { input_path: input, output_path: output };
}

function verifyEngine(policy) {
  if (process.versions.node !== policy.engine.node_version) {
    reject("ENGINE_INCOMPATIBLE", `expected Node ${policy.engine.node_version}`);
  }
  let api, installedVersion;
  try {
    api = require(ENGINE_PATH);
    installedVersion = require(path.join(ENGINE_PATH, "package.json")).version;
  } catch {
    reject("ENGINE_INCOMPATIBLE", "pinned ag-psd is not installed in the local research environment");
  }
  if (installedVersion !== policy.engine.version) {
    reject("ENGINE_INCOMPATIBLE", "ag-psd version does not match the frozen contract");
  }
  return api;
}

function preflight(request) {
  const policyBytes = fs.readFileSync(POLICY_PATH);
  const policy = JSON.parse(policyBytes);
  if (!policy.preflight_only || policy.public_mutation_authorized !== false ||
      policy.research_admission.mode !== "exact_known_fixture_sha256_only") {
    reject("EXECUTION_FAILED", "unsafe or unexpected mutation policy configuration");
  }
  if (!request || typeof request !== "object" || Array.isArray(request)) {
    reject("INVALID_ARGUMENT", "request must be one JSON object");
  }
  const keys = Object.keys(request);
  if (keys.length !== REQUEST_FIELDS.length ||
      keys.some(key => !REQUEST_FIELDS.includes(key))) {
    reject("INVALID_ARGUMENT", "request must contain exactly the frozen contract fields");
  }
  if (request.contract_version !== policy.schema_version) {
    reject("INVALID_ARGUMENT", "unsupported mutation preflight contract version");
  }
  if (request.operation !== "rename") {
    reject("UNSUPPORTED_OPERATION", "only layer rename is a research candidate");
  }
  const input = path.resolve(requireString(request, "input_path"));
  const output = requireString(request, "output_path");
  const expectedSource = assertSha(
    requireString(request, "expected_source_sha256"), "expected_source_sha256"
  );
  const layerId = requireString(request, "layer_id");
  if (!/^L\d{4,}$/.test(layerId) || canonicalId(Number(layerId.slice(1))) !== layerId) {
    reject("INVALID_ARGUMENT", "layer_id must be canonical pre-order L0001 format");
  }
  const expectedOldName = requireString(request, "expected_old_name");
  const newName = requireString(request, "new_name");
  const newNameCodepoints = Array.from(newName);
  if (newNameCodepoints.length > 80 || newName.trim() !== newName ||
      newName.normalize("NFC") !== newName || /[\u0000-\u001f\u007f]/.test(newName)) {
    reject("INVALID_ARGUMENT", "new_name must be NFC, 1-80 code points, no outer whitespace or controls");
  }
  if (newName === expectedOldName) {
    reject("INVALID_ARGUMENT", "rename must change the target layer name");
  }

  let stat;
  try { stat = fs.lstatSync(input); }
  catch { reject("INVALID_INPUT", "PSD input cannot be read"); }
  if (!stat.isFile() || stat.size < 26 ||
      stat.size > policy.research_admission.max_source_bytes) {
    reject("UNSUPPORTED_DOCUMENT", "source must be a bounded regular file (not a symlink)");
  }
  let bytes;
  try { bytes = fs.readFileSync(input); }
  catch { reject("INVALID_INPUT", "PSD input cannot be read"); }
  const actualHash = sha256(bytes);
  if (actualHash !== expectedSource) {
    reject("SOURCE_VERSION_CONFLICT", "input bytes differ from expected_source_sha256");
  }

  // Byte-identical known-fixture identity is the *only* admission path.
  // These exact bytes were in the prior M4c-1 evidence. Parsing normalized
  // metadata alone cannot certify lossless handling of opaque PSD blocks.
  const fixture = policy.known_fixtures.find(item => item.sha256 === actualHash);
  if (!fixture) {
    reject("UNSUPPORTED_DOCUMENT",
      "arbitrary files blocked: opaque metadata/embedded content not independently audited");
  }
  const version = bytes.readUInt16BE(4);
  const format = version === 1 ? "psd" : version === 2 ? "psb" : null;
  const width = bytes.readUInt32BE(18);
  const height = bytes.readUInt32BE(14);
  const bitDepth = bytes.readUInt16BE(22);
  const colorMode = bytes.readUInt16BE(24);
  if (bytes.toString("ascii", 0, 4) !== "8BPS" ||
      format !== fixture.format || bitDepth !== 8 || colorMode !== 3 ||
      width !== fixture.width || height !== fixture.height ||
      !Number.isSafeInteger(width * height) ||
      width * height > policy.research_admission.max_pixels ||
      path.extname(input).toLowerCase() !== `.${fixture.format}`) {
    reject("UNSUPPORTED_DOCUMENT", "PSD header/geometry/color mode outside approved fixture profile");
  }
  const targetPaths = validateDestination(input, output, format);
  const ag = verifyEngine(policy);
  let psd;
  try {
    psd = ag.readPsd(bytes, {
      useRawData: true,
      useRawThumbnail: true,
      skipCompositeImageData: true,
      skipLinkedFilesData: true,
      logMissingFeatures: false
    });
  } catch {
    reject("UNSUPPORTED_DOCUMENT", "ag-psd could not safely inspect the approved input");
  }
  const layers = flattenedLayers(psd, policy.research_admission.max_layers);
  if (layers.length !== fixture.layer_count) {
    reject("UNSUPPORTED_DOCUMENT", "unexpected logical layer tree");
  }
  const selected = layers.find(layer => layer.id === layerId);
  const approved = fixture.research_target;
  if (!selected || approved.layer_id !== layerId ||
      approved.name !== selected.name || approved.kind !== selected.kind ||
      !policy.research_admission.allowed_layer_kinds.includes(selected.kind) ||
      selected.has_mask || selected.has_effects) {
    reject("UNSUPPORTED_DOCUMENT", "selected layer is not the reviewed fixture target");
  }
  if (expectedOldName !== selected.name) {
    reject("SOURCE_VERSION_CONFLICT", "expected_old_name differs from the current selected layer");
  }

  return {
    contract_version: policy.schema_version,
    operation: "psd.layer.rename",
    status: "research_candidate",
    write_authorized: false,
    dry_run: true,
    source: {
      path: targetPaths.input_path,
      sha256: actualHash,
      fixture_profile: fixture.id,
      format,
      width,
      height,
      bits_per_channel: bitDepth,
      color_mode: "rgb",
      layer_count: layers.length
    },
    target: {
      layer_id: selected.id,
      kind: selected.kind,
      expected_old_name: selected.name,
      new_name: newName
    },
    output: { path: targetPaths.output_path, policy: "new_file_only" },
    engine: policy.engine,
    policy_sha256: sha256(policyBytes),
    required_before_public_write: [
      "independent_editor_round_trip",
      "opaque_block_and_linked_asset_preservation",
      "strict_staged_post_write_conformance",
      "M4b_version_bound_source_and_output_receipt",
      "separate_production_authorization"
    ],
    side_effects: "none"
  };
}

function main() {
  try {
    if (process.argv.length !== 2) {
      reject("INVALID_ARGUMENT", "research preflight reads one JSON request on stdin; no CLI flags");
    }
    const input = fs.readFileSync(0, "utf8");
    if (!input || Buffer.byteLength(input) > 32 * 1024) {
      reject("INVALID_ARGUMENT", "request JSON must be non-empty and at most 32 KiB");
    }
    let request;
    try { request = JSON.parse(input); }
    catch { reject("INVALID_ARGUMENT", "request is not valid JSON"); }
    process.stdout.write(JSON.stringify({ status: "ok", plan: preflight(request) }) + "\n");
  } catch (error) {
    const code = error instanceof PreflightError ? error.code : "EXECUTION_FAILED";
    process.stdout.write(JSON.stringify({
      status: "error",
      error: { code, message: error.message || String(error) }
    }) + "\n");
    process.exitCode = ["EXECUTION_FAILED"].includes(code) ? 1 :
      ["UNSUPPORTED_OPERATION", "UNSUPPORTED_DOCUMENT", "ENGINE_INCOMPATIBLE"].includes(code) ? 3 : 2;
  }
}

module.exports = { preflight, PreflightError };
if (require.main === module) main();
