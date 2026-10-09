"use strict";

const assert = require("node:assert/strict");
const { test } = require("node:test");
const fs = require("node:fs");
const path = require("node:path");
const {
  buildEvidence, variableLengthPlan, rawNames, independentRead, layerDelta,
} = require("./m4c6_dual_name_fidelity.cjs");
const { sortedNames } = require("./m4c6_photoshop_readonly.cjs");

const ROOT = path.resolve(__dirname,"../../../../");
const POLICY = require(path.join(ROOT,"docs/data/psd-safe-mutation-scope-m4c2-v1.json"));
const SNAPSHOT = require(path.join(ROOT,"docs/data/m4c6-dual-name-fidelity-v1.json"));
const PHOTOSHOP = require(path.join(ROOT,"docs/data/m4c6-photoshop-name-receipt-v1.json"));
const filename=f=>path.join(ROOT,f.path);

test("ag-psd/psd-tools 1.20.0 independent Unicode-name and preview evidence reproduces",()=>{
  assert.equal(process.versions.node,"22.23.3");
  assert.equal(buildEvidence(),fs.readFileSync(
    path.join(ROOT,"docs/data/m4c6-dual-name-fidelity-v1.json"),"utf8"));
  assert.equal(SNAPSHOT.summary.fixtures,4);
  assert.equal(SNAPSHOT.summary.both_parsers_agree_on_selected_name,4);
  assert.equal(SNAPSHOT.summary.independent_composite_equal,4);
  assert.equal(SNAPSHOT.summary.legacy_unicode_divergence,3);
  assert.equal(SNAPSHOT.summary.variable_length_proposals_only,4);
  assert.equal(SNAPSHOT.summary.photoshop_application_fidelity_verified,0);
  assert.equal(SNAPSHOT.summary.production_writes_authorized,0);
  for(const c of SNAPSHOT.cases){
    assert.equal(c.psd_tools_only_selected_logical_name_changed,true);
    assert.equal(c.psd_tools_target_name_matches,true);
    assert.equal(c.source_unchanged,true);
    assert.equal(c.raw_other_bytes_preserved,true);
    assert.equal(c.psd_tools_decoded_preview_comparison.equal,true);
    assert.equal(c.editor_fidelity_certified,false);
    assert.equal(c.public_write_authorized,false);
  }
});

test("different-length UTF-16 names require changing PSD lengths and are not implemented",()=>{
  for(const f of POLICY.known_fixtures){
    const c=SNAPSHOT.cases.find(c=>c.fixture===f.id);
    const p=SNAPSHOT.variable_length_plans.find(c=>c.fixture===f.id);
    assert.equal(p.layer_id,c.layer_id);
    assert.equal(p.status,"plan_only_not_implemented");
    assert.equal(p.proposed_write_authorized,false);
    assert.equal(p.requires_parent_length_rebase,true);
    assert.equal(p.requires_relocating_downstream_blocks,true);
    assert.equal(p.layer_and_mask_delta_bytes,p.layer_info_delta_bytes);
    assert.equal(p.layer_extra_delta_bytes,p.layer_info_delta_bytes);
    assert.ok(p.luni_payload_delta_bytes>0);
  }
});

test("raw Unicode and legacy Pascal names are separately identified",()=>{
  for(const f of POLICY.known_fixtures){
    const c=SNAPSHOT.cases.find(c=>c.fixture===f.id);
    assert.ok(c);
    const x=rawNames(filename(f),c.source_raw_names.raw_index);
    assert.deepEqual(x,c.source_raw_names);
    assert.equal(x.unicode_name,c.original_name);
  }
  const group=SNAPSHOT.cases.find(x=>x.fixture==="nested-group");
  assert.equal(group.candidate_raw_names.legacy_pascal_ascii_equal,true);
  assert.equal(group.candidate_raw_names.unicode_name,"Group 2");
});

test("Photoshop receipt is scoped to non-saving layer-name reads only",()=>{
  assert.equal(PHOTOSHOP.operation,"open/read_layer_names/close_without_saving");
  assert.equal(PHOTOSHOP.application,"Adobe Photoshop 2026 on macOS");
  assert.equal(PHOTOSHOP.summary.fixtures,4);
  assert.equal(PHOTOSHOP.summary.read_verified,2);
  assert.equal(PHOTOSHOP.summary.unavailable_or_mismatch,2);
  assert.equal(PHOTOSHOP.summary.saved_or_modified_inputs,0);
  assert.equal(PHOTOSHOP.summary.production_writes_authorized,0);
  assert.equal(PHOTOSHOP.visual_or_save_roundtrip_certified,false);
  assert.ok(PHOTOSHOP.cases.every(c=>
    c.all_input_and_candidate_bytes_unchanged_by_photoshop &&
    !c.photoshop_save_or_visual_roundtrip_verified &&
    !c.production_write_authorized));

  for(const id of ["simple-psd","simple-psb"]){
    const c=PHOTOSHOP.cases.find(c=>c.fixture===id);
    assert.equal(c.status,"source_name_not_as_expected");
    const background=c.photoshop_candidate_layers.find(x=>x.kind==="background");
    assert.ok(background);
    assert.equal(background.name,"背景");
    assert.deepEqual(c.photoshop_source_layers,c.photoshop_candidate_layers);
  }
  for(const id of ["nested-group","duplicate-names"]){
    const c=PHOTOSHOP.cases.find(c=>c.fixture===id);
    assert.equal(c.status,"read_verified");
    assert.equal(c.photoshop_layer_name_read_verified,true);
    assert.notDeepEqual(sortedNames(c.photoshop_source_layers),
      sortedNames(c.photoshop_candidate_layers));
  }
});

test("cross-parser reader does not silently accept a malformed PSD, and layer mismatch fails",()=>{
  const f=POLICY.known_fixtures[0];
  const source=independentRead(filename(f));
  assert.ok(source.layers.length>0);
  assert.equal(source.parser_version,"1.20.0");
  assert.equal(layerDelta(source.layers,source.layers,"L0001","Another"),false);
  const edited=source.layers.map(x=>({...x}));
  edited[0].name="wrong target";
  assert.equal(layerDelta(source.layers,edited,"L0002","wrong target"),false);
  const expected=source.layers.map(x=>({...x}));
  expected[0].name="New";
  assert.equal(layerDelta(source.layers,expected,"L0001","New"),true);
});
