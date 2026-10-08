"use strict";

// M4c-1 research-only PSD round-trip probe. NOT a YuTool runtime entrypoint.
// The fixture allow-list is committed and the script never writes source PSDs.

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const crypto = require("node:crypto");
const assert = require("node:assert/strict");

const ROOT = path.resolve(__dirname, "../../../../");
const OUTPUT_ROOT = path.join(ROOT, "target", "m4c1-psd-mutation");
const REPORT = path.join(OUTPUT_ROOT, "evidence.json");
const ENGINE_ROOT = path.join(ROOT, "packaging", "ag-psd-engine", "node_modules", "ag-psd");

const FIXTURES = [
  ["simple-psd", "fixtures/psd/upstream/psd-tools/2layers.psd", "simple"],
  ["simple-psb", "fixtures/psd/upstream/psd-tools/2layers.psb", "psb"],
  ["nested-group", "fixtures/psd/upstream/psd-tools/group.psd", "groups"],
  ["duplicate-names", "fixtures/psd/derived/duplicate-layer-names.psd", "duplicate_names"],
  ["text-layer", "fixtures/psd/upstream/psd-tools/type-layer.psd", "text"],
  ["layer-masks", "fixtures/psd/upstream/psd-tools/masks-2.psd", "masks"],
  ["advanced-blending", "fixtures/psd/benchmark/upstream/psd-tools/advanced-blending.psd", "blend_modes"],
  ["layer-effects", "fixtures/psd/benchmark/upstream/psd-tools/layer-effects.psd", "effects"],
  ["smart-object", "fixtures/psd/benchmark/upstream/psd-tools/placed-layer.psd", "smart_object"],
  ["baseline-opacity", "fixtures/psd/benchmark/upstream/psd-tools/background-red-opacity-80.psd", "raster"],
  ["high-bit-psd", "fixtures/psd/benchmark/upstream/psd-tools/posterize-16bit-rgb.psd", "high_bit"],
  ["high-bit-psb", "fixtures/psd/benchmark/upstream/psd-tools/32bit.psb", "high_bit_psb"],
];
const OPERATIONS = ["noop", "rename", "visibility", "opacity"];

function sha(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function fail(message) {
  throw new Error(message);
}

if (process.versions.node !== "22.23.3") {
  fail(`This pinned probe requires Node.js 22.23.3, got ${process.versions.node}`);
}
const ag = require(ENGINE_ROOT);
const version = require(path.join(ENGINE_ROOT, "package.json")).version;
if (version !== "31.0.2") fail(`Expected ag-psd 31.0.2, got ${version}`);

// ag-psd's useImageData mode avoids premultiplication through a browser canvas.
// Do not initialize a rendering canvas or silently flatten a PSD.
ag.initializeCanvas(
  () => { throw new Error("full canvas rendering is not allowed in M4c-1"); },
  (width, height) => {
    const pixels = width * height;
    if (!Number.isSafeInteger(pixels) || pixels <= 0 || pixels > 2_000_000) {
      fail("probe bitmap exceeds 2M pixels");
    }
    return { width, height, data: new Uint8ClampedArray(pixels * 4) };
  }
);

const READ_OPTIONS = {
  useImageData: true,
  useRawThumbnail: true,
  skipLinkedFilesData: true,
  logMissingFeatures: false,
};

function read(buffer) {
  const psd = ag.readPsd(buffer, READ_OPTIONS);
  if (psd.width <= 0 || psd.height <= 0 || psd.width * psd.height > 2_000_000) {
    fail("probe document exceeds pixel budget");
  }
  return psd;
}

function hashImageData(imageData) {
  if (!imageData || !imageData.data) return null;
  const { data } = imageData;
  return sha(Buffer.from(data.buffer, data.byteOffset, data.byteLength));
}

function digestObject(value) {
  if (value === undefined) return null;
  return sha(Buffer.from(JSON.stringify(value)));
}

function flatten(psd) {
  const output = [];
  function walk(children, parent = null, depth = 1) {
    if (!children) return;
    for (const layer of children) {
      const index = output.length + 1;
      const id = `L${String(index).padStart(4, "0")}`;
      output.push({
        id, depth, parent,
        name: layer.name ?? "",
        visible: layer.hidden !== true,
        opacity: layer.opacity ?? 1,
        kind: layer.children ? "group" : layer.text ? "text" :
          layer.placedLayer ? "smart_object" : "pixel",
        bounds: [layer.left ?? null, layer.top ?? null, layer.right ?? null, layer.bottom ?? null],
        blendMode: layer.blendMode ?? null,
        pixelHash: hashImageData(layer.imageData),
        pixelMaskHash: hashImageData(layer.mask && layer.mask.imageData),
        vectorMaskDigest: digestObject(layer.vectorMask),
        effectsDigest: digestObject(layer.effects),
        textDigest: digestObject(layer.text),
        placedLayerDigest: digestObject(layer.placedLayer),
        childrenCount: layer.children?.length ?? 0,
      });
      walk(layer.children, id, depth + 1);
    }
  }
  walk(psd.children);
  return output;
}

function snapshot(psd) {
  const resources = psd.imageResources || {};
  return {
    header: {
      width: psd.width, height: psd.height,
      bitsPerChannel: psd.bitsPerChannel ?? 8,
      colorMode: psd.colorMode ?? null,
    },
    compositeHash: hashImageData(psd.imageData),
    thumbnailRawHash: digestObject(resources.thumbnailRaw),
    resourceKeys: Object.keys(resources).sort(),
    resourceDigests: Object.fromEntries(
      Object.entries(resources).map(([key, value]) => [key, digestObject(value)])
    ),
    layerTree: flatten(psd),
  };
}

function diffPaths(left, right, prefix = "", output = []) {
  if (output.length >= 12) return output;
  if (Object.is(left, right)) return output;
  if (!left || !right || typeof left !== "object" || typeof right !== "object") {
    output.push(prefix || "$");
    return output;
  }
  const keys = new Set([...Object.keys(left), ...Object.keys(right)]);
  for (const key of keys) {
    if (output.length >= 12) break;
    diffPaths(left[key], right[key], `${prefix}.${key}`, output);
  }
  return output;
}

function findLayerByIndex(psd, targetIndex) {
  let index = 0;
  function visit(children) {
    for (const layer of children || []) {
      index++;
      if (index === targetIndex) return layer;
      const nested = visit(layer.children);
      if (nested) return nested;
    }
  }
  return visit(psd.children);
}

function probeOne(input, fixture, operation, runDir) {
  const sourceBuffer = fs.readFileSync(input);
  const sourceHash = sha(sourceBuffer);
  const result = {
    fixture: fixture[0],
    feature: fixture[2],
    operation,
    source_sha256: sourceHash,
    source_bytes: sourceBuffer.length,
    status: "not_run",
  };
  try {
    if (sourceBuffer.length > 8 * 1024 * 1024) fail("fixture too large");
    const psd = read(sourceBuffer);
    const before = snapshot(psd);
    result.format = fixture[1].endsWith(".psb") ? "psb" : "psd";
    result.bits_per_channel = before.header.bitsPerChannel;
    result.layer_count = before.layerTree.length;
    const chosen = before.layerTree.findIndex(l => l.kind !== "group");
    const selectedIndex = fixture[0] === "duplicate-names" ? 1 :
      fixture[0] === "nested-group" && operation === "rename" ?
        before.layerTree.findIndex(l => l.kind === "group") : chosen;
    if (selectedIndex < 0 || selectedIndex >= before.layerTree.length) {
      fail("fixture has no selectable user-facing layer");
    }
    const selected = findLayerByIndex(psd, selectedIndex + 1);
    const prior = before.layerTree[selectedIndex];
    result.layer_id = prior.id;
    result.layer_kind = prior.kind;
    result.expected_change = operation === "noop" ? null :
      operation === "rename" ? "name" :
        operation === "visibility" ? "visible" : "opacity";
    let expected;
    if (operation === "rename") {
      expected = "YuTool M4c1 Rename";
      selected.name = expected;
    } else if (operation === "visibility") {
      expected = !prior.visible;
      selected.hidden = !expected;
    } else if (operation === "opacity") {
      expected = 0.37;
      selected.opacity = expected;
    }

    const bytes = ag.writePsdBuffer(psd, { psb: result.format === "psb" });
    if (!Buffer.isBuffer(bytes) || bytes.length < 26) fail("writer emitted invalid buffer");
    const output = path.join(runDir, `${fixture[0]}-${operation}.${result.format}`);
    fs.writeFileSync(output, bytes, { flag: "wx" });
    const diskBytes = fs.readFileSync(output);
    if (sha(diskBytes) !== sha(bytes)) fail("disk output differs from encoder buffer");
    const after = snapshot(read(diskBytes));
    result.output_bytes = diskBytes.length;
    result.output_sha256 = sha(diskBytes);
    result.byte_equal_to_source = result.output_sha256 === result.source_sha256;
    result.composite_preserved = before.compositeHash === after.compositeHash;
    result.thumbnail_preserved = before.thumbnailRawHash === after.thumbnailRawHash;
    result.resource_keys_preserved =
      JSON.stringify(before.resourceKeys) === JSON.stringify(after.resourceKeys);
    result.resource_digests_preserved =
      JSON.stringify(before.resourceDigests) === JSON.stringify(after.resourceDigests);

    const observed = after.layerTree[selectedIndex];
    result.mutation_applied = operation === "noop" ? true :
      operation === "rename" ? observed?.name === expected :
        operation === "visibility" ? observed?.visible === expected :
          Math.abs((observed?.opacity ?? NaN) - expected) <= (1 / 255 + 1e-6);

    const normalized = structuredClone(before);
    if (operation !== "noop" && after.layerTree[selectedIndex]) {
      normalized.layerTree[selectedIndex][result.expected_change] =
        after.layerTree[selectedIndex][result.expected_change];
    }
    result.unexpected_layer_or_header_changes = diffPaths(
      { header: normalized.header, layers: normalized.layerTree },
      { header: after.header, layers: after.layerTree }
    );
    result.structure_preserved_except_target = result.unexpected_layer_or_header_changes.length === 0;
    result.observed_invariants_passed =
      result.mutation_applied && result.structure_preserved_except_target &&
      result.composite_preserved && result.resource_keys_preserved &&
      result.resource_digests_preserved &&
      result.thumbnail_preserved;
    // ag-psd explicitly does not recompute Photoshop's cached composition for
    // changes that affect visual output. Preserving stale pixels is NOT safety.
    result.visual_change_requires_composite_refresh =
      operation === "visibility" || operation === "opacity";
    result.stale_composite_risk =
      result.visual_change_requires_composite_refresh && !!before.compositeHash &&
      result.composite_preserved;
    result.restricted_rename_candidate =
      operation === "rename" &&
      ["simple-psd", "simple-psb", "nested-group", "duplicate-names"].includes(fixture[0]) &&
      result.observed_invariants_passed;
    // Test results never authorize public writes; Photoshop application-level
    // fidelity and unknown/opaque resource preservation remain unverified.
    result.public_mutation_authorized = false;
    result.status = "written_and_reparsed";
  } catch (error) {
    result.status = "failed_closed";
    result.error = error instanceof Error ? error.message : String(error);
  }
  // A test that changes a source fixture, even on the error path, is invalid.
  if (sha(fs.readFileSync(input)) !== sourceHash) fail(`source fixture mutated: ${input}`);
  result.source_unchanged = true;
  return result;
}

function main() {
  const fixtureRoot = path.join(ROOT, "fixtures", "psd");
  if (!fs.existsSync(fixtureRoot)) fail("fixture corpus is missing");
  fs.mkdirSync(OUTPUT_ROOT, { recursive: true });
  const runDir = fs.mkdtempSync(path.join(OUTPUT_ROOT, "run-"));
  const trials = [];
  for (const fixture of FIXTURES) {
    const source = path.join(ROOT, fixture[1]);
    if (!fs.existsSync(source)) fail(`missing fixture: ${fixture[1]}`);
    for (const operation of OPERATIONS) trials.push(probeOne(source, fixture, operation, runDir));
  }
  const report = {
    schema_version: "1",
    milestone: "M4c-1",
    experimental_only: true,
    engine: { id: "ag-psd", version },
    runtime: { node: process.versions.node, platform: os.platform(), arch: os.arch() },
    fixture_count: FIXTURES.length,
    trial_count: trials.length,
    summary: {
      written_and_reparsed: trials.filter(t => t.status === "written_and_reparsed").length,
      failed_closed: trials.filter(t => t.status === "failed_closed").length,
      composite_changed: trials.filter(t => t.status === "written_and_reparsed" && !t.composite_preserved).length,
      structure_changed: trials.filter(t => t.status === "written_and_reparsed" && !t.structure_preserved_except_target).length,
      resources_changed: trials.filter(t => t.status === "written_and_reparsed" && !t.resource_digests_preserved).length,
      mutation_not_applied: trials.filter(t => t.status === "written_and_reparsed" && !t.mutation_applied).length,
      observed_invariants_passed: trials.filter(t => t.observed_invariants_passed).length,
      stale_composite_risk: trials.filter(t => t.stale_composite_risk).length,
      restricted_rename_candidates: trials.filter(t => t.restricted_rename_candidate).length,
      public_mutation_authorized: trials.filter(t => t.public_mutation_authorized).length,
    },
    trials,
  };
  fs.writeFileSync(REPORT, JSON.stringify(report, null, 2) + "\n", { flag: "w" });
  console.log(JSON.stringify({
    ...report.summary,
    fixture_count: report.fixture_count,
    trial_count: report.trial_count,
    report: path.relative(ROOT, REPORT),
  }));
  assert.equal(trials.length, FIXTURES.length * OPERATIONS.length);
  assert.ok(trials.every(t => t.source_unchanged));
  assert.equal(report.summary.failed_closed, 8, "16/32-bit write rejection must be explicit");
  assert.equal(report.summary.written_and_reparsed, 40);
  assert.equal(report.summary.public_mutation_authorized, 0);
  for (const operation of OPERATIONS) {
    const highBitTrials = trials.filter(t => t.feature.startsWith("high_bit") && t.operation === operation);
    assert.equal(highBitTrials.length, 2);
    assert.ok(highBitTrials.every(t =>
      t.status === "failed_closed" &&
      t.error?.includes("bitsPerChannel other than 8 are not supported for writing")
    ));
  }
}

if (require.main === module) main();
