"use strict";

const assert = require("node:assert/strict");
const { test } = require("node:test");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const crypto = require("node:crypto");
const { scan, inspectFile, ScanError } = require("./m4c3_raw_block_inventory.cjs");
const { buildReport } = require("./m4c3_raw_inventory_corpus.cjs");

const ROOT = path.resolve(__dirname, "../../../../");
const SCRIPT = path.join(__dirname, "m4c3_raw_block_inventory.cjs");
const sha = bytes => crypto.createHash("sha256").update(bytes).digest("hex");
const base = f => path.join(ROOT, "fixtures/psd", f);
const source = f => fs.readFileSync(base(f));
const SIMPLE = "upstream/psd-tools/2layers.psd";
const PSB = "upstream/psd-tools/2layers.psb";
const COMPLEX = "benchmark/upstream/psd-tools/advanced-blending.psd";
const HIGH = "benchmark/upstream/psd-tools/32bit.psb";
const corpusFiles = [
  "upstream/psd-tools/2layers.psd", "upstream/psd-tools/2layers.psb",
  "upstream/psd-tools/group.psd", "derived/duplicate-layer-names.psd",
  "upstream/psd-tools/type-layer.psd", "upstream/psd-tools/masks-2.psd",
  "benchmark/upstream/psd-tools/background-red-opacity-80.psd",
  "benchmark/upstream/psd-tools/advanced-blending.psd",
  "benchmark/upstream/psd-tools/masks2.psd",
  "benchmark/upstream/psd-tools/layer-effects.psd",
  "benchmark/upstream/psd-tools/placed-layer.psd",
  "benchmark/upstream/psd-tools/posterize-16bit-rgb.psd",
  "benchmark/upstream/psd-tools/32bit.psb",
];

function expectFailure(buffer, code = "INVALID_INPUT") {
  assert.throws(() => scan(buffer), error =>
    error instanceof ScanError && error.code === code,
    `expected ${code}`
  );
}

test("M4c-3 scans 13 committed valid PSD/PSB examples with no mutation permission", () => {
  for (const fixture of corpusFiles) {
    const bytes = source(fixture);
    const before = sha(bytes);
    const inventory = scan(bytes);
    assert.equal(inventory.status, "structurally_parsed");
    assert.equal(inventory.mutation_authorized, false);
    assert.equal(inventory.safe_to_rewrite, false);
    assert.equal(inventory.source.sha256, before);
    assert.equal(inventory.source.bytes, bytes.length);
    assert.ok(inventory.sections.some(x => x.name === "image_resources"));
    assert.ok(inventory.sections.some(x => x.name === "layer_and_mask_info"));
    assert.ok(inventory.merged_image.bytes >= 2);
    assert.ok(inventory.summary.image_resource_count >= 1);
    assert.ok(inventory.summary.opaque_payload_bytes >= 1);
    assert.ok(inventory.risks.includes("opaque_payload_not_proven_roundtrip_safe"));
    assert.equal(sha(bytes), before);
  }
});

test("committed corpus inventory is deterministic and denies mutation for all inputs", () => {
  const reportFile = path.join(ROOT, "docs/data/psd-raw-block-inventory-m4c3-v1.json");
  assert.equal(buildReport(), fs.readFileSync(reportFile, "utf8"));
  const report = JSON.parse(buildReport());
  assert.equal(report.total_fixtures, 14);
  assert.equal(report.scanned, 13);
  assert.equal(report.rejected, 1);
  assert.ok(report.with_unknown_blocks >= 1);
  assert.ok(report.cases.every(x => !x.mutation_authorized));
});

test("simple document inventories Image Resources and exact tagged-block offsets", () => {
  const x = scan(source(SIMPLE));
  assert.equal(x.header.format, "psd");
  assert.equal(x.header.bit_depth, 8);
  assert.equal(x.header.color_mode, 3);
  assert.equal(x.summary.layer_record_count, 2);
  assert.equal(x.summary.image_resource_count, 2);
  assert.deepEqual(x.image_resources.map(r => r.id), [1005, 1024]);
  assert.deepEqual(x.tagged_blocks.map(r => r.key), ["luni", "luni"]);
  assert.equal(x.summary.unknown_block_count, 0);
  assert.ok(x.risks.includes("absent_global_layer_mask_length"));
  for (const record of [...x.tagged_blocks, ...x.image_resources]) {
    assert.match(record.data_sha256, /^[0-9a-f]{64}$/);
    assert.ok(record.offset >= 26);
  }
  assert.ok(x.layer_records[0].channels.length > 0);
  assert.match(x.layer_records[0].channels[0].data_sha256, /^[0-9a-f]{64}$/);
  assert.match(x.merged_image.data_sha256, /^[0-9a-f]{64}$/);
  assert.ok(x.sections.every(section => /^[0-9a-f]{64}$/.test(section.data_sha256)));
});

test("PSB parses 64-bit section lengths and long-key tagged blocks", () => {
  const psb = scan(source(PSB));
  assert.equal(psb.header.version, 2);
  assert.equal(psb.header.format, "psb");
  assert.equal(psb.summary.layer_record_count, 2);
  assert.equal(psb.sections.find(x => x.name === "layer_info").length_field_offset + 8,
    psb.sections.find(x => x.name === "layer_info").offset);
  const high = scan(source(HIGH));
  assert.equal(high.header.bit_depth, 32);
  assert.ok(high.tagged_blocks.some(x => x.key === "Lr32" && x.length_width === 8));
  assert.ok(high.tagged_blocks.some(x => x.signature === "8B64"));
  assert.ok(high.risks.includes("embedded_layer_records_not_recursively_decoded"));
  assert.ok(high.risks.includes("high_bit_depth_not_writable"));
});

test("unknown resource IDs are inventoried and never treated as write-safe", () => {
  const bytes = Buffer.from(source(SIMPLE));
  const record = scan(bytes).image_resources[0];
  bytes.writeUInt16BE(0xeeee, record.offset + 4);
  const result = scan(bytes);
  assert.ok(result.unknown_blocks.some(x =>
    x.category === "image_resource" && x.id === 0xeeee
  ));
  assert.equal(result.safe_to_rewrite, false);
  assert.ok(result.risks.includes("unknown_metadata"));
});

test("unknown Additional Layer Information keys are recorded without skipping their bytes", () => {
  const bytes = Buffer.from(source(SIMPLE));
  const first = scan(bytes).tagged_blocks[0];
  bytes.write("YUzz", first.offset + 4, 4, "ascii");
  const result = scan(bytes);
  assert.equal(result.summary.unknown_block_count, 1);
  assert.equal(result.unknown_blocks[0].category, "tagged_block");
  assert.equal(result.unknown_blocks[0].key, "YUzz");
  assert.equal(result.tagged_blocks[0].length, first.length);
  assert.equal(result.safe_to_rewrite, false);
});

test("malformed signatures, truncated sections and lengths fail closed", () => {
  const initial = source(SIMPLE);
  expectFailure(source("malformed/truncated-header.psd"));
  // The composite is deliberately *opaque*. Truncating its compressed tail
  // cannot be proven from section boundaries and requires a pixel decoder.
  for (const count of [0, 1, 5, 26, 31, 70, 125, 155]) {
    expectFailure(initial.subarray(0, count));
  }
  let bytes = Buffer.from(initial);
  bytes.write("NOPE", 0, 4, "ascii");
  expectFailure(bytes);
  bytes = Buffer.from(initial);
  bytes.writeUInt16BE(99, 4);
  expectFailure(bytes, "UNSUPPORTED_DOCUMENT");
  bytes = Buffer.from(initial);
  bytes[6] = 1;
  expectFailure(bytes);
  bytes = Buffer.from(initial);
  bytes.writeUInt32BE(0xfffffff0, 26);
  expectFailure(bytes);
  bytes = Buffer.from(initial);
  bytes.writeUInt32BE(0xfffffff0, 30);
  expectFailure(bytes);
  bytes = Buffer.from(initial);
  const resourceOffset = scan(bytes).image_resources[0].offset;
  bytes.write("XXIM", resourceOffset, 4, "ascii");
  expectFailure(bytes);
  bytes = Buffer.from(initial);
  bytes.writeUInt32BE(0xfffffff0, resourceOffset + 8);
  expectFailure(bytes);
  bytes = Buffer.from(initial);
  const tag = scan(bytes).tagged_blocks[0];
  bytes.write("XXXX", tag.offset, 4, "ascii");
  expectFailure(bytes);
  bytes = Buffer.from(initial);
  bytes.writeUInt32BE(0xfffffff0, tag.offset + 8);
  expectFailure(bytes);
});

test("PSB 64-bit lengths beyond JS safe-integer bound are rejected", () => {
  let bytes = Buffer.from(source(PSB));
  const section = scan(bytes).sections.find(x => x.name === "layer_and_mask_info");
  bytes.writeBigUInt64BE(BigInt(Number.MAX_SAFE_INTEGER) + 1n, section.length_field_offset);
  expectFailure(bytes, "UNSUPPORTED_DOCUMENT");
  bytes = Buffer.from(source(HIGH));
  const long = scan(bytes).tagged_blocks.find(x => x.key === "Lr32");
  assert.ok(long);
  bytes.writeBigUInt64BE(BigInt(Number.MAX_SAFE_INTEGER) + 1n, long.offset + 8);
  expectFailure(bytes, "UNSUPPORTED_DOCUMENT");
});

test("length fuzzing remains bounded and does not throw raw Buffer range errors", () => {
  const orig = source(SIMPLE);
  const sample = scan(orig);
  const sectionFields = sample.sections
    .filter(x => ["color_mode_data", "image_resources", "layer_and_mask_info", "layer_info"].includes(x.name))
    .map(x => x.length_field_offset);
  for (const offset of sectionFields) {
    for (const huge of [0xfffffffe, 0xffffffff]) {
      const bytes = Buffer.from(orig);
      bytes.writeUInt32BE(huge, offset);
      expectFailure(bytes);
    }
  }
  const overBudget = Buffer.alloc(8 * 1024 * 1024 + 1);
  expectFailure(overBudget, "UNSUPPORTED_DOCUMENT");
});

test("input files remain unchanged; CLI returns structured errors for symlink and malformed PSD", () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "yu-m4c3-inventory-"));
  try {
    const sourceFile = base(SIMPLE);
    const copy = path.join(tmp, "copy.psd");
    fs.copyFileSync(sourceFile, copy);
    const before = sha(fs.readFileSync(copy));
    const good = spawnSync(process.execPath, [SCRIPT, copy], {
      cwd: ROOT, encoding: "utf8", maxBuffer: 1024 * 1024,
    });
    assert.equal(good.status, 0, good.stderr);
    const body = JSON.parse(good.stdout);
    assert.equal(body.status, "ok");
    assert.equal(body.inventory.mutation_authorized, false);
    assert.equal(sha(fs.readFileSync(copy)), before);
    const link = path.join(tmp, "input-link.psd");
    fs.symlinkSync(copy, link);
    const denied = spawnSync(process.execPath, [SCRIPT, link], {
      cwd: ROOT, encoding: "utf8",
    });
    assert.equal(denied.status, 3);
    assert.equal(JSON.parse(denied.stdout).error.code, "UNSUPPORTED_DOCUMENT");
    const broken = spawnSync(process.execPath, [SCRIPT, base("malformed/truncated-header.psd")], {
      cwd: ROOT, encoding: "utf8",
    });
    assert.equal(broken.status, 2);
    assert.equal(JSON.parse(broken.stdout).error.code, "INVALID_INPUT");
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});
