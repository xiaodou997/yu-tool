"use strict";

const assert = require("node:assert/strict");
const { test } = require("node:test");
const path = require("node:path");
const fs = require("node:fs");

const ROOT = path.resolve(__dirname, "../../../../");
const HISTORICAL = require(path.join(ROOT,
  "docs/data/m4c7c-photoshop-control-v2.json"));
const NATIVE = require(path.join(ROOT,
  "docs/data/m4c7c-photoshop-native-reacceptance-v3.json"));
const FROZEN = require(path.join(ROOT,
  "docs/data/psd-rename-scope-m4c7-v1.json"));

test("Photoshop reacceptance records real saveAs/reopen but no fidelity certification",()=>{
  assert.equal(NATIVE.schema_version,"1");
  assert.equal(NATIVE.public_write_authorized,false);
  assert.equal(NATIVE.summary.admitted_cases,1);
  assert.equal(NATIVE.summary.native_two_outputs_saved,1);
  assert.equal(NATIVE.summary.native_name_reopen_verified,1);
  assert.equal(NATIVE.summary.strict_byte_fidelity_verified,0);
  assert.equal(NATIVE.summary.paired_research_save_reopen_passed,0);
  assert.equal(NATIVE.summary.product_write_authorizations,0);
  const x=NATIVE.cases[0];
  assert.equal(x.fixture,"nested-group");
  assert.equal(x.status,"native_name_reopen_verified_fidelity_blocked");
  assert.equal(x.control_saved,true);
  assert.equal(x.candidate_saved,true);
  assert.equal(x.native_name_save_reopen_verified,true);
  assert.equal(x.strict_byte_fidelity_verified,false);
  assert.equal(x.source_and_experimental_candidate_unchanged,true);
  assert.equal(x.save_roundtrip_research_passed,false);
  assert.equal(x.public_write_authorized,false);
});

test("Photoshop original-control and edited-candidate names persist independently",()=>{
  const x=NATIVE.cases[0];
  assert.ok(x.photoshop_native_control_names.some(z=>
    z.kind==="group"&&z.name==="Group 1"));
  assert.ok(x.photoshop_native_candidate_names.some(z=>
    z.kind==="group"&&z.name==="Group 2"));
  assert.equal(x.editor_control_name_preserved,true);
  assert.equal(x.editor_candidate_name_preserved,true);
  assert.equal(x.control_semantic_tree_preserved,true);
  assert.equal(x.candidate_semantic_tree_preserved,true);
  assert.equal(x.independent_preview_saved_control_vs_saved_candidate_equal,true);
  assert.equal(x.independent_preview_control_preserved,false);
  assert.equal(x.independent_preview_candidate_preserved,false);
  assert.equal(x.source_to_photoshop_save_preview_changed,true);
  assert.equal(x.output_control.bytes,33256);
  assert.equal(x.output_candidate.bytes,33240);
  assert.match(x.output_control.sha256,/^[0-9a-f]{64}$/);
  assert.match(x.output_candidate.sha256,/^[0-9a-f]{64}$/);
});

test("unchanged compressed channels/composite do not mask opaque metadata differences",()=>{
  const x=NATIVE.cases[0];
  const raw=x.saved_pair_strict_raw_evidence;
  assert.equal(raw.old_unicode_name,"Group 1");
  assert.equal(raw.candidate_unicode_name,"Group 2");
  assert.equal(raw.old_pascal_name,"Group 1");
  assert.equal(raw.candidate_pascal_name,"Group 2");
  assert.deepEqual(raw.channels,{added:[],removed:[],changed:[]});
  assert.equal(raw.merged_composite_equal,true);
  assert.equal(raw.header_equal,true);
  assert.ok(raw.resources.changed.length>0);
  assert.ok(raw.tags.changed.some(x=>x.includes("shmd")));
  assert.ok(raw.byte_difference_proof.changed_non_name_byte_count>0);
  assert.equal(raw.byte_difference_proof.file_size_equal,false);
  assert.equal(raw.strict_saved_pair_bytes_passed,false);
  assert.equal(FROZEN.public_write_authorized,false);
});

test("old blocked receipt remains unchanged in meaning and never becomes a native pass",()=>{
  assert.equal(HISTORICAL.status,"blocked_native_application_unresponsive");
  assert.equal(HISTORICAL.native_preflight.ready,false);
  assert.equal(HISTORICAL.summary.paired_research_save_reopen_passed,0);
  assert.ok(HISTORICAL.cases.every(c=>
    c.save_command_attempted===false &&
    !c.control_saved && !c.candidate_saved &&
    !c.public_write_authorized));
  const files=fs.readdirSync(path.join(ROOT,"docs/data"));
  assert.ok(files.includes("m4c7c-photoshop-control-v2.json"));
  assert.ok(files.includes("m4c7c-photoshop-native-reacceptance-v3.json"));
});
