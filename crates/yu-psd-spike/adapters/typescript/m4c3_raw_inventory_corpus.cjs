"use strict";

// Deterministic report generator / verifier for the committed PSD and PSB
// corpus. No source PSD modification, no image decode, no write authorization.

const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const { inspectFile, ScanError } = require("./m4c3_raw_block_inventory.cjs");

const ROOT = path.resolve(__dirname, "../../../../");
const REPORT = path.join(ROOT, "docs/data/psd-raw-block-inventory-m4c3-v1.json");
const SOURCES = [
  ["fixtures/psd", "fixtures/psd/corpus.json"],
  ["fixtures/psd/benchmark", "fixtures/psd/benchmark/corpus.json"],
];

function buildReport() {
  const cases = [];
  for (const [root, filename] of SOURCES) {
    const manifest = JSON.parse(fs.readFileSync(path.join(ROOT, filename), "utf8"));
    for (const fixture of manifest.fixtures) {
      const filepath = path.join(ROOT, root, fixture.path);
      const relative = path.relative(ROOT, filepath).split(path.sep).join("/");
      try {
        const v = inspectFile(filepath);
        if (fixture.expected?.parse === "reject") {
          throw new Error(`malformed fixture unexpectedly parsed: ${fixture.id}`);
        }
        cases.push({
          fixture: fixture.id, path: relative, outcome: "structurally_parsed",
          sha256: v.source.sha256, file_bytes: v.source.bytes,
          format: v.header.format, bit_depth: v.header.bit_depth,
          color_mode: v.header.color_mode, top_level_layer_records: v.summary.layer_record_count,
          image_resource_count: v.summary.image_resource_count,
          additional_block_count: v.summary.tagged_block_count,
          unknown_block_count: v.summary.unknown_block_count,
          unknown_resources: [...new Set(v.unknown_blocks
            .filter(x => x.category === "image_resource").map(x => x.id))].sort((a,b)=>a-b),
          unknown_layer_keys: [...new Set(v.unknown_blocks
            .filter(x => x.category === "tagged_block").map(x => x.key))].sort(),
          risks: v.risks,
          mutation_authorized: false,
        });
      } catch (error) {
        if (!(error instanceof ScanError) || fixture.expected?.parse !== "reject") {
          throw error;
        }
        cases.push({
          fixture: fixture.id, path: relative, outcome: "rejected",
          error_code: error.code, mutation_authorized: false,
        });
      }
    }
  }
  cases.sort((a,b) => a.fixture.localeCompare(b.fixture, "en"));
  const results = {
    schema_version: "1", milestone: "M4c-3", engine: "byte-scanner (no PSD writer)",
    mutation_authorized: false,
    total_fixtures: cases.length,
    scanned: cases.filter(x=>x.outcome==="structurally_parsed").length,
    rejected: cases.filter(x=>x.outcome==="rejected").length,
    with_unknown_blocks: cases.filter(x=>x.unknown_block_count > 0).length,
    cases,
  };
  assert.equal(results.total_fixtures, 14);
  assert.equal(results.scanned, 13);
  assert.equal(results.rejected, 1);
  assert.equal(results.mutation_authorized, false);
  return JSON.stringify(results, null, 2) + "\n";
}

if (require.main === module) {
  const mode = process.argv[2];
  if (!["--write", "--check"].includes(mode) || process.argv.length !== 3) {
    process.stderr.write("usage: node m4c3_raw_inventory_corpus.cjs --check|--write\n");
    process.exit(2);
  }
  const actual = buildReport();
  if (mode === "--write") {
    fs.writeFileSync(REPORT, actual);
  } else {
    const existing = fs.readFileSync(REPORT, "utf8");
    assert.equal(actual, existing, "committed M4c-3 evidence differs from current corpus");
  }
  const obj = JSON.parse(actual);
  process.stdout.write(JSON.stringify({
    mode, total: obj.total_fixtures, scanned: obj.scanned, rejected: obj.rejected,
    with_unknown_blocks: obj.with_unknown_blocks,
  }) + "\n");
}

module.exports = { buildReport };
