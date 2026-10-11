"use strict";

// Research-only M4c-7a admission. This NEVER grants actual write permission.
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const assert = require("node:assert/strict");
const { scan } = require("./m4c3_raw_block_inventory.cjs");
const { decodeLuni, utf16be } = require("./m4c5_byte_patch_spike.cjs");
const { rawNames } = require("./m4c6_dual_name_fidelity.cjs");

const ROOT = path.resolve(__dirname, "../../../../");
const POLICY = require(path.join(ROOT, "docs/data/psd-rename-scope-m4c7-v1.json"));
const M4C2 = require(path.join(ROOT, "docs/data/psd-safe-mutation-scope-m4c2-v1.json"));
const NATIVE = require(path.join(ROOT, "docs/data/m4c6-photoshop-name-receipt-v1.json"));
const sha = b => crypto.createHash("sha256").update(b).digest("hex");

function assess(config) {
  const fixture = M4C2.known_fixtures.find(f => f.id === config.fixture);
  const native = NATIVE.cases.find(c => c.fixture === config.fixture);
  if (!fixture || !native) throw Error("unknown research fixture");
  const file = path.join(ROOT, fixture.path);
  const bytes = fs.readFileSync(file);
  if (sha(bytes) !== fixture.sha256) throw Error("frozen source sha mismatch");
  const inv = scan(bytes);
  const raw = rawNames(file, config.raw_index);
  const tags = inv.tagged_blocks.filter(t => t.scope === "layer" &&
    t.layer_index === config.raw_index && t.key === "luni");
  if (tags.length !== 1) throw Error("target lacks unique raw Unicode name");
  const old = decodeLuni(bytes,tags[0]);
  if(raw.unicode_name!==fixture.research_target.name ||
      old.value!==fixture.research_target.name) {
    throw Error("raw PSD layer name and expected target mismatch");
  }
  if (config.layer_id !== fixture.research_target.layer_id) throw Error("wrong canonical layer id");
  const reasons = [];
  const targetNative = native.photoshop_source_layers?.find(x =>
    x.name === fixture.research_target.name);
  if (!targetNative && native.photoshop_source_layers?.some(x => x.kind === "background")) {
    reasons.push("native_Background_name_not_reported");
  }
  if (!native.photoshop_layer_name_read_verified) reasons.push("native_Photoshop_read_failed");
  if (!raw.legacy_pascal_ascii_equal) reasons.push("legacy_Pascal_mismatch");
  if (!/^[\x20-\x7e]+$/.test(config.proposed_name) ||
      !/^[\x20-\x7e]+$/.test(old.value)) reasons.push("non_ASCII_legacy_name_not_proven");
  if (Buffer.byteLength(config.proposed_name,"ascii") !==
      Buffer.byteLength(old.value,"ascii")) reasons.push("legacy_name_length_not_equal");
  if (utf16be(config.proposed_name).length !== old.units*2) {
    reasons.push("UTF16_code_unit_length_not_equal");
  }
  if (!POLICY.candidate_kinds.includes(fixture.research_target.kind)) {
    reasons.push("layer_kind_denied");
  }
  if (native.photoshop_source_layers?.find(x => x.kind === "background" &&
      x.name === fixture.research_target.name)) reasons.push("native_Background_denied");
  const research = reasons.length === 0;
  return {
    fixture:fixture.id,source_sha256:fixture.sha256,
    layer_id:config.layer_id,raw_index:config.raw_index,
    kind:fixture.research_target.kind,
    before:old.value,after:config.proposed_name,
    raw_legacy_ascii_matches_unicode:raw.legacy_pascal_ascii_equal,
    native_name_read_verified:native.photoshop_layer_name_read_verified,
    gate:research?"research_save_reopen_candidate":"blocked",
    reasons,
    public_write_authorized:false,
  };
}

function buildReport() {
  const results = POLICY.known_research_cases.map(assess);
  for (let i=0;i<results.length;i++) {
    assert.equal(results[i].gate,POLICY.known_research_cases[i].gate,
      "research scope gate unexpected");
  }
  return {
    schema_version:"1",milestone:"M4c-7a",
    public_write_authorized:false,
    summary:{
      examined:results.length,
      research_save_reopen_candidates:results.filter(x=>
        x.gate==="research_save_reopen_candidate").length,
      blocked:results.filter(x=>x.gate==="blocked").length,
      production_writes_authorized:0,
    },
    cases:results,
  };
}
module.exports={assess,buildReport,POLICY};
