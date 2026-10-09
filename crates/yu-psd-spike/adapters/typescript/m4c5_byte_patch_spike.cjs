"use strict";

// M4c-5: fixed-width PSD/PSB layer-name byte-patch research only.
// Accepts NO user-supplied files or arbitrary destinations. Writes only
// create-new disposable copies under ignored target/m4c5-psd-byte-patch/.
// A successful experiment NEVER constitutes production-write authorization.

const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const assert = require("node:assert/strict");
const { scan } = require("./m4c3_raw_block_inventory.cjs");
const { preflight } = require("./m4c2_safe_mutation_preflight.cjs");

const ROOT = path.resolve(__dirname, "../../../../");
const OUTPUT_ROOT = path.join(ROOT, "target", "m4c5-psd-byte-patch");
const REPORT_PATH = path.join(ROOT, "docs/data/m4c5-psd-byte-patch-evidence-v1.json");
const POLICY = require(path.join(ROOT, "docs/data/psd-safe-mutation-scope-m4c2-v1.json"));
const AG_PATH = path.join(ROOT, "packaging/ag-psd-engine/node_modules/ag-psd");
const sha = data => crypto.createHash("sha256").update(data).digest("hex");

const CASES = [
  { fixture: "simple-psd", new_name: "Дом", raw_index: 0 },
  { fixture: "simple-psb", new_name: "Дом", raw_index: 0 },
  { fixture: "nested-group", new_name: "Group 2", raw_index: 3 },
  { fixture: "duplicate-names", new_name: "Y", raw_index: 1 },
];

function refuse(reason) {
  const error = new Error(reason);
  error.code = "RESEARCH_REFUSAL";
  throw error;
}

function loadAg() {
  if (process.versions.node !== "22.23.3") refuse("requires pinned Node 22.23.3");
  let ag, version;
  try {
    ag = require(AG_PATH);
    version = require(path.join(AG_PATH, "package.json")).version;
  } catch { refuse("pinned ag-psd not installed"); }
  if (version !== "31.0.2") refuse("requires pinned ag-psd 31.0.2");
  return ag;
}

function utf16be(value) {
  if (typeof value !== "string" || !value || value.normalize("NFC") !== value ||
      /[\u0000-\u001f\u007f]/.test(value) ||
      Array.from(value).some(ch => ch.length === 1 && /[\ud800-\udfff]/.test(ch))) {
    refuse("new_name must be nonempty, NFC, without control or invalid surrogate characters");
  }
  const b = Buffer.from(value, "utf16le");
  b.swap16();
  return b;
}

function decodeLuni(buffer, tag) {
  if (tag.key !== "luni" || tag.signature !== "8BIM" || tag.length_width !== 4 ||
      tag.length < 4) refuse("target has no valid Unicode layer-name block");
  const payload = tag.offset + 12;
  if (payload + tag.length > buffer.length) refuse("out-of-range layer-name payload");
  const units = buffer.readUInt32BE(payload);
  if (units < 1 || units > 80 || 4 + units * 2 > tag.length) {
    refuse("Unicode layer name has an invalid UTF-16BE unit count");
  }
  const start = payload + 4, end = start + units * 2;
  const value = Buffer.from(buffer.subarray(start, end)).swap16().toString("utf16le");
  if (utf16be(value).length !== units * 2) refuse("invalid source UTF-16 layer name");
  return { value, units, start, end, tail_bytes: tag.length - 4 - units * 2 };
}

function legacyPascalPosition(buffer, record, version) {
  // Independently calculate the embedded Pascal-name range from bounded raw
  // layer-record framing; no name search or file-global string replacement.
  const count = record.channel_count;
  const prefix = record.offset + 16 + 2 + count * (version === 2 ? 10 : 6) + 12;
  if (prefix + 12 > buffer.length) refuse("invalid raw layer record position");
  const extraLength = buffer.readUInt32BE(prefix);
  const extraStart = prefix + 4, extraEnd = extraStart + extraLength;
  if (extraEnd > buffer.length) refuse("layer extra section exceeds source");
  let pos = extraStart;
  if (pos + 4 > extraEnd) refuse("missing mask length");
  const mask = buffer.readUInt32BE(pos);
  pos += 4 + mask;
  if (pos + 4 > extraEnd) refuse("mask section exceeds layer extra");
  const blend = buffer.readUInt32BE(pos);
  pos += 4 + blend;
  if (pos + 1 > extraEnd) refuse("missing legacy Pascal name");
  const n = buffer[pos];
  if (pos + 1 + n > extraEnd) refuse("legacy Pascal name exceeds layer extra");
  return { start: pos + 1, end: pos + 1 + n, value: buffer.toString("latin1", pos + 1, pos + 1 + n) };
}

function flatten(psd) {
  const arr = [];
  function go(children) {
    for (const layer of children || []) {
      arr.push({
        id: "L" + String(arr.length + 1).padStart(4, "0"),
        name: layer.name || "",
        group: Array.isArray(layer.children),
        bounds: [layer.left ?? null, layer.top ?? null, layer.right ?? null, layer.bottom ?? null],
      });
      go(layer.children);
    }
  }
  go(psd.children);
  return arr;
}

function mapByteRange(original, modified) {
  if (original.length !== modified.length) refuse("fixed-width patch changed file size");
  const positions = [];
  for (let i = 0; i < original.length; i++) {
    if (original[i] !== modified[i]) positions.push(i);
  }
  return positions;
}

function createCandidate(source, fixture, request, rawIndex, ag) {
  const before = scan(source);
  if (before.source.sha256 !== fixture.sha256 || before.header.format !== fixture.format) {
    refuse("raw inventory no longer matches research fixture");
  }
  const record = before.layer_records[rawIndex];
  const tags = before.tagged_blocks.filter(t =>
    t.scope === "layer" && t.layer_index === rawIndex && t.key === "luni"
  );
  if (!record || tags.length !== 1) refuse("target raw layer must have exactly one Unicode name block");
  const name = decodeLuni(source, tags[0]);
  if (name.value !== request.expected_old_name) refuse("raw Unicode name does not match expected old name");
  const encoded = utf16be(request.new_name);
  if (encoded.length !== name.end - name.start) {
    refuse("research patch requires identical UTF-16BE code-unit length");
  }
  const unicodeBefore = Buffer.from(source.subarray(name.start, name.end));
  const patched = Buffer.from(source);
  encoded.copy(patched, name.start);
  const touched = [{ category: "unicode_luni_name", offset: name.start, bytes: encoded.length }];
  const legacy = legacyPascalPosition(source, record, before.header.version);
  let legacyUpdated = false;
  // Keep ASCII duplicate name representations in sync only when the legacy
  // original agrees with the selected Unicode name and lengths are equal.
  if (/^[\x20-\x7e]+$/.test(name.value) &&
      /^[\x20-\x7e]+$/.test(request.new_name) &&
      legacy.value === name.value &&
      legacy.end - legacy.start === Buffer.byteLength(request.new_name, "ascii")) {
    patched.write(request.new_name, legacy.start, "ascii");
    legacyUpdated = true;
    touched.push({ category: "legacy_pascal_name", offset: legacy.start, bytes: legacy.end - legacy.start });
  }
  const changedPositions = mapByteRange(source, patched);
  if (!changedPositions.length) refuse("patch made no actual name changes");
  for (const pos of changedPositions) {
    if (!touched.some(t => pos >= t.offset && pos < t.offset + t.bytes)) {
      refuse("patched an unauthorized PSD byte");
    }
  }

  const after = scan(patched);
  assert.equal(after.source.bytes, before.source.bytes);
  assert.deepEqual(after.header, before.header);
  assert.deepEqual(after.merged_image, before.merged_image);
  assert.deepEqual(after.image_resources, before.image_resources);
  assert.equal(after.unknown_blocks.length, before.unknown_blocks.length);
  assert.deepEqual(after.layer_records.map(r => r.channels), before.layer_records.map(r => r.channels));
  const changedTags = [];
  assert.equal(after.tagged_blocks.length, before.tagged_blocks.length);
  for (let i = 0; i < before.tagged_blocks.length; i++) {
    const a = before.tagged_blocks[i], b = after.tagged_blocks[i];
    if (a.data_sha256 !== b.data_sha256) {
      changedTags.push({ scope: a.scope, layer_index: a.layer_index, key: a.key });
      if (!(a.key === "luni" && a.scope === "layer" && a.layer_index === rawIndex)) {
        refuse("non-target tagged data changed");
      }
    }
  }
  if (changedTags.length !== 1) refuse("unexpected tagged-block delta count");
  const readOptions = {
    skipLayerImageData: true,
    skipCompositeImageData: true,
    skipThumbnail: true,
    skipLinkedFilesData: true,
    logMissingFeatures: false,
  };
  const logicalBefore = flatten(ag.readPsd(source, readOptions));
  const logicalAfter = flatten(ag.readPsd(patched, readOptions));
  if (logicalBefore.length !== logicalAfter.length) refuse("logical layer count changed");
  const selected = logicalBefore.findIndex(x => x.id === request.layer_id);
  if (selected < 0 || logicalBefore[selected].name !== request.expected_old_name ||
      logicalBefore[selected].group !== (fixture.research_target.kind === "group")) {
    refuse("canonical PSD target identity is not independently confirmed");
  }
  for (let i = 0; i < logicalBefore.length; i++) {
    const expected = { ...logicalBefore[i] };
    if (i === selected) expected.name = request.new_name;
    if (JSON.stringify(expected) !== JSON.stringify(logicalAfter[i])) {
      refuse("ag-psd round-trip layer metadata differs beyond selected name");
    }
  }
  return {
    bytes: patched,
    evidence: {
      raw_record_index: rawIndex,
      logical_layer_id: request.layer_id,
      original_unicode_name: name.value,
      updated_unicode_name: request.new_name,
      name_utf16_code_units: name.units,
      original_legacy_pascal_name: legacy.value,
      legacy_pascal_updated: legacyUpdated,
      legacy_pascal_may_be_stale: !legacyUpdated,
      changed_byte_count: changedPositions.length,
      change_ranges: touched,
      modified_tagged_blocks: changedTags,
      raw_resources_equal: true,
      raw_layer_channels_equal: true,
      raw_merged_composite_equal: true,
      logical_tree_except_requested_name_equal: true,
      source_sha256: before.source.sha256,
      candidate_sha256: after.source.sha256,
      source_bytes: source.length,
      candidate_bytes: patched.length,
      mutation_authorized: false,
    },
  };
}

function runCase(config, outputDir, ag) {
  const fixture = POLICY.known_fixtures.find(x => x.id === config.fixture);
  if (!fixture || !CASES.some(x =>
    x.fixture === config.fixture && x.new_name === config.new_name && x.raw_index === config.raw_index)) {
    refuse("case does not belong to the frozen experimental corpus");
  }
  const sourcePath = path.join(ROOT, fixture.path);
  const outputPath = path.join(outputDir, config.fixture + "." + fixture.format);
  const request = {
    contract_version: "1",
    operation: "rename",
    input_path: sourcePath,
    expected_source_sha256: fixture.sha256,
    layer_id: fixture.research_target.layer_id,
    expected_old_name: fixture.research_target.name,
    new_name: config.new_name,
    output_path: outputPath,
  };
  const plan = preflight(request);
  assert.equal(plan.write_authorized, false);
  assert.equal(plan.source.sha256, fixture.sha256);
  const source = fs.readFileSync(sourcePath);
  assert.equal(sha(source), fixture.sha256);
  const { bytes, evidence } = createCandidate(source, fixture, request, config.raw_index, ag);
  fs.writeFileSync(outputPath, bytes, { flag: "wx" });
  const disk = fs.readFileSync(outputPath);
  assert.equal(sha(disk), evidence.candidate_sha256);
  assert.deepEqual(disk, bytes);
  assert.equal(sha(fs.readFileSync(sourcePath)), fixture.sha256);
  return {
    fixture: fixture.id,
    status: "research_byte_patch_verified",
    read_only_preflight: true,
    disposable_output_only: true,
    ...evidence,
    candidate_reopened_and_byte_verified: true,
    public_write_authorized: false,
  };
}

function buildReport() {
  const ag = loadAg();
  fs.mkdirSync(OUTPUT_ROOT, { recursive: true });
  const output = fs.mkdtempSync(path.join(OUTPUT_ROOT, "run-"));
  const cases = CASES.map(config => runCase(config, output, ag));
  const summary = {
    tested: cases.length,
    research_byte_patches_verified: cases.filter(x =>
      x.status === "research_byte_patch_verified").length,
    source_files_unchanged: cases.filter(x => x.source_bytes === x.candidate_bytes).length,
    legacy_name_representation_stale: cases.filter(x => x.legacy_pascal_may_be_stale).length,
    production_writes_authorized: 0,
  };
  assert.equal(summary.tested, 4);
  assert.equal(summary.research_byte_patches_verified, 4);
  assert.equal(summary.production_writes_authorized, 0);
  return JSON.stringify({
    schema_version: "1",
    milestone: "M4c-5",
    scope: "research-only fixed-width luni edits; not general PSD writer",
    engine: { id: "ag-psd", version: "31.0.2", node_version: process.versions.node },
    public_write_authorized: false,
    summary,
    cases,
  }, null, 2) + "\n";
}

if (require.main === module) {
  try {
    const mode = process.argv[2];
    if (!["--write", "--check"].includes(mode) || process.argv.length !== 3) {
      refuse("usage: node m4c5_byte_patch_spike.cjs --write|--check");
    }
    const report = buildReport();
    if (mode === "--write") fs.writeFileSync(REPORT_PATH, report);
    else assert.equal(fs.readFileSync(REPORT_PATH, "utf8"), report,
      "committed M4c-5 evidence differs from current experiment");
    process.stdout.write(JSON.stringify({ mode, ...JSON.parse(report).summary }) + "\n");
  } catch (error) {
    process.stderr.write((error && error.message) || String(error));
    process.exitCode = 1;
  }
}

module.exports = { CASES, createCandidate, runCase, buildReport, decodeLuni, utf16be, refuse };
