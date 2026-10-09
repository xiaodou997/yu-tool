"use strict";

// Cross-parser PSD names + experimental source-preserving patch receipt.
// Never writes to a user's PSD path, never declares Photoshop validation.
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const crypto = require("node:crypto");
const { spawnSync } = require("node:child_process");
const assert = require("node:assert/strict");
const { scan } = require("./m4c3_raw_block_inventory.cjs");
const { CASES, runCase, decodeLuni, utf16be } = require("./m4c5_byte_patch_spike.cjs");

const ROOT = path.resolve(__dirname, "../../../../");
const POLICY = require(path.join(ROOT, "docs/data/psd-safe-mutation-scope-m4c2-v1.json"));
const AG_ROOT = path.join(ROOT, "packaging/ag-psd-engine/node_modules/ag-psd");
const PYTHON = process.env.M4C6_PYTHON || path.join(ROOT, "target/m4c6-py312/bin/python");
const PY_READER = path.join(ROOT, "crates/yu-psd-spike/adapters/m4c6_psd_tools_reader.py");
const EVIDENCE = path.join(ROOT, "docs/data/m4c6-dual-name-fidelity-v1.json");
const TEMP = path.join(ROOT, "target/m4c6-dual-name-fidelity");
const sha = data => crypto.createHash("sha256").update(data).digest("hex");

function independentRead(filename) {
  const result = spawnSync(PYTHON, [PY_READER, filename], {
    cwd: ROOT, encoding: "utf8", timeout: 30000, maxBuffer: 1024 * 1024,
  });
  if (result.error || result.status !== 0) {
    throw Error("psd-tools reader failed: " + (result.error?.message || result.stderr || result.stdout));
  }
  const parsed = JSON.parse(result.stdout);
  if (parsed.status !== "ok") throw Error("psd-tools rejected file: " + parsed.message);
  return parsed.document;
}

function rawNames(filename, rawIndex) {
  const bytes = fs.readFileSync(filename);
  const inventory = scan(bytes);
  const record = inventory.layer_records[rawIndex];
  const tagged = inventory.tagged_blocks.filter(x =>
    x.scope === "layer" && x.layer_index === rawIndex && x.key === "luni"
  );
  if (!record || tagged.length !== 1) throw Error("ambiguous target raw Unicode-name record");
  const name = decodeLuni(bytes, tagged[0]);
  return {
    raw_index: rawIndex,
    unicode_name: name.value,
    unicode_code_units: name.units,
    unicode_bytes: name.end - name.start,
    legacy_pascal_bytes: record.legacy_name,
    legacy_pascal_ascii_equal: /^[\x20-\x7e]*$/.test(record.legacy_name) &&
      record.legacy_name === name.value,
    luni_offset: tagged[0].offset,
  };
}

function abstractLayerNames(layers) {
  return layers.map(x => ({ id: x.id, name: x.name, group: x.kind === "group" }));
}

function layerDelta(before, after, layerId, nextName) {
  if (before.length !== after.length) return false;
  let modifiedCount = 0;
  for (let i = 0; i < before.length; i++) {
    if (before[i].id !== after[i].id || before[i].kind !== after[i].kind) return false;
    if (before[i].name !== after[i].name) {
      modifiedCount++;
      if (before[i].id !== layerId || after[i].name !== nextName) return false;
    }
  }
  return modifiedCount === 1;
}

function variableLengthPlan(fixture, rawIndex, candidateName) {
  const file = path.join(ROOT, fixture.path);
  const inventory = scan(fs.readFileSync(file));
  const matching = inventory.tagged_blocks.filter(x =>
    x.scope === "layer" && x.layer_index === rawIndex && x.key === "luni");
  if (matching.length !== 1) throw Error("no unique target luni");
  const previous = decodeLuni(fs.readFileSync(file), matching[0]);
  const newEncoded = utf16be(candidateName);
  const delta = newEncoded.length - (previous.end - previous.start);
  const beforePad = matching[0].length % 2, afterPad = (matching[0].length + delta) % 2;
  const paddedDelta = delta + afterPad - beforePad;
  const nonzero = paddedDelta !== 0;
  return {
    proposed_name: candidateName,
    original_utf16_code_units: previous.units,
    proposed_utf16_code_units: newEncoded.length / 2,
    luni_payload_delta_bytes: delta,
    layer_extra_delta_bytes: paddedDelta,
    layer_info_delta_bytes: paddedDelta,
    layer_and_mask_delta_bytes: paddedDelta,
    requires_parent_length_rebase: nonzero,
    requires_relocating_downstream_blocks: nonzero,
    legacy_pascal_rewrite_unresolved: true,
    proposed_write_authorized: false,
    status: "plan_only_not_implemented",
  };
}

function buildEvidence() {
  if (process.versions.node !== "22.23.3") throw Error("pinned Node.js 22.23.3 required");
  const ag = require(AG_ROOT);
  if (require(path.join(AG_ROOT, "package.json")).version !== "31.0.2") {
    throw Error("pinned ag-psd 31.0.2 required");
  }
  fs.mkdirSync(TEMP, {recursive:true});
  const outputDir = fs.mkdtempSync(path.join(TEMP,"run-"));
  const results = [];
  for (const choice of CASES) {
    const fixture = POLICY.known_fixtures.find(f => f.id === choice.fixture);
    assert.ok(fixture);
    const sourceFile = path.join(ROOT, fixture.path);
    const patch = runCase(choice, outputDir, ag);
    const outputFile = path.join(outputDir, fixture.id + "." + fixture.format);
    const sourceRef = independentRead(sourceFile), patchedRef = independentRead(outputFile);
    const beforeRaw = rawNames(sourceFile, choice.raw_index),
      afterRaw = rawNames(outputFile, choice.raw_index);
    const selected = patchedRef.layers.find(x => x.id === fixture.research_target.layer_id);
    const psdToolsNameValid = selected?.name === choice.new_name;
    const namesOnlyTargetChanged = layerDelta(sourceRef.layers, patchedRef.layers,
      fixture.research_target.layer_id, choice.new_name);
    const previewA = sourceRef.independent_preview;
    const previewB = patchedRef.independent_preview;
    const independentPreviewEqual = previewA.status === "rendered" &&
      previewB.status === "rendered" &&
      previewA.rgba_sha256 === previewB.rgba_sha256;
    const agExpected = patch.updated_unicode_name === choice.new_name;
    results.push({
      fixture: fixture.id,
      format: fixture.format,
      source_sha256: fixture.sha256,
      candidate_sha256: patch.candidate_sha256,
      layer_id: fixture.research_target.layer_id,
      kind: fixture.research_target.kind,
      original_name: fixture.research_target.name,
      updated_name: choice.new_name,
      source_raw_names: beforeRaw,
      candidate_raw_names: afterRaw,
      ag_psd_expected_name: agExpected,
      psd_tools_target_name_matches: psdToolsNameValid,
      psd_tools_only_selected_logical_name_changed: namesOnlyTargetChanged,
      psd_tools_decoded_preview_comparison: {
        source_status: previewA.status,
        candidate_status: previewB.status,
        equal: independentPreviewEqual,
      },
      raw_other_bytes_preserved: patch.raw_resources_equal &&
        patch.raw_layer_channels_equal && patch.raw_merged_composite_equal,
      source_unchanged: sha(fs.readFileSync(sourceFile)) === fixture.sha256,
      photoshop_test_status: "not_executed",
      editor_fidelity_certified: false,
      public_write_authorized: false,
    });
  }
  const proposals = POLICY.known_fixtures.map(f => {
    const choice = CASES.find(x=>x.fixture===f.id);
    const altered = choice.new_name + " Longer";
    return {
      fixture:f.id,layer_id:f.research_target.layer_id,
      ...variableLengthPlan(f,choice.raw_index,altered)
    };
  });
  const summary = {
    fixtures:results.length,
    both_parsers_agree_on_selected_name:results.filter(x=>x.ag_psd_expected_name &&
      x.psd_tools_target_name_matches &&
      x.psd_tools_only_selected_logical_name_changed).length,
    independent_composite_equal:results.filter(x=>
      x.psd_tools_decoded_preview_comparison.equal).length,
    legacy_unicode_divergence:results.filter(x=>
      !x.candidate_raw_names.legacy_pascal_ascii_equal).length,
    variable_length_proposals_only:proposals.length,
    photoshop_application_fidelity_verified:0,
    production_writes_authorized:0,
  };
  const report = {
    schema_version:"1", milestone:"M4c-6",
    engines:{ ag_psd:"31.0.2", psd_tools:"1.20.0", node:"22.23.3", python:"3.12" },
    platform:os.platform(), arch:os.arch(),
    status:"cross_parser_research",
    public_write_authorized:false,summary,
    cases:results,variable_length_plans:proposals,
  };
  assert.equal(summary.fixtures,4);
  assert.equal(summary.production_writes_authorized,0);
  return JSON.stringify(report,null,2)+"\n";
}

if (require.main === module) {
  const mode = process.argv[2];
  if (!["--write","--check"].includes(mode) || process.argv.length !== 3) {
    process.stderr.write("usage: node m4c6_dual_name_fidelity.cjs --write|--check\n");
    process.exit(2);
  }
  const report=buildEvidence();
  if(mode==="--write")fs.writeFileSync(EVIDENCE,report);
  else assert.equal(fs.readFileSync(EVIDENCE,"utf8"),report);
  console.log(JSON.stringify({mode,...JSON.parse(report).summary}));
}
module.exports={buildEvidence,rawNames,variableLengthPlan,independentRead,layerDelta};
