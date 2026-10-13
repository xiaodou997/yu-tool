"use strict";

const assert=require("node:assert/strict");
const {test}=require("node:test");
const path=require("node:path");
const ROOT=path.resolve(__dirname,"../../../../");
const CORPUS=require(path.join(ROOT,
  "docs/data/m4c7e-readonly-corpus-profile-v1.json"));
const NATIVE=require(path.join(ROOT,
  "docs/data/m4c7e-native-readonly-samples-v1.json"));
const DECISION=require(path.join(ROOT,
  "docs/data/m4c7e-release-decision-v1.json"));
const POLICY=require(path.join(ROOT,
  "docs/data/psd-rename-scope-m4c7-v1.json"));
const {check:checkCorpus}=require("./m4c7e_corpus_admission.cjs");
const {check:checkNative}=require("./m4c7e_native_readonly_sampling.cjs");

test("13 independent readonly frozen PSD/PSB corpus profiles remain unapproved",()=>{
  checkCorpus(CORPUS);
  assert.equal(CORPUS.summary.scanned,13);
  assert.equal(CORPUS.summary.unadmitted_to_public_writes,13);
  assert.equal(CORPUS.summary.production_writes_authorized,0);
  assert.ok(CORPUS.cases.every(x=>!x.public_write_authorized&&!x.mutation_authorized));
  assert.ok(CORPUS.cases.some(x=>x.format==="psb"));
  assert.ok(CORPUS.cases.some(x=>x.bit_depth===16));
  assert.ok(CORPUS.cases.some(x=>x.bit_depth===32));
  assert.ok(CORPUS.cases.some(x=>x.layer_kinds.includes("smartobject")));
  assert.ok(CORPUS.cases.some(x=>x.layer_kinds.includes("type")));
});
test("existing low-risk normal pixel candidate is NOT added to writer allow-list",()=>{
  const candidate=CORPUS.cases.filter(x=>
    x.next_research_classification.startsWith("candidate_for"));
  assert.equal(candidate.length,1);
  assert.equal(candidate[0].fixture,"bench-baseline-rgb");
  assert.equal(candidate[0].existing_m4c7a_gate,"not_in_exact_hash_research_scope");
  assert.equal(candidate[0].public_write_authorized,false);
  assert.equal(POLICY.known_research_cases.filter(x=>
    x.gate==="research_save_reopen_candidate").length,1);
  assert.equal(POLICY.public_write_authorized,false);
});
test("three actual native Photoshop READ-ONLY profile checks keep all sources unchanged",()=>{
  checkNative(NATIVE);
  assert.equal(NATIVE.summary.requested_readonly_fixtures,3);
  assert.equal(NATIVE.summary.inspected,3);
  assert.equal(NATIVE.summary.source_copies_unchanged,3);
  assert.equal(NATIVE.summary.production_writes_authorized,0);
  assert.ok(NATIVE.cases.every(x=>
    x.status==="photoshop_read_only_inspected"&&
    x.source_and_scratch_unchanged&&
    x.photoshop_save_attempted===false&&
    x.native_and_reference_layer_count_equal));
  const pixel=NATIVE.cases.find(x=>x.fixture==="bench-baseline-rgb");
  assert.equal(pixel.native_background_count,0);
  assert.deepEqual(pixel.native_photoshop_layers.map(x=>x.name).sort(),
    ["Layer 0","Layer 1"]);
  const cyrillic=NATIVE.cases.find(x=>x.fixture==="simple-pixel-layers-psd");
  assert.equal(cyrillic.native_background_count,1);
  assert.ok(cyrillic.native_photoshop_layers.some(x=>
    x.kind==="background"&&x.name==="背景"));
  const text=NATIVE.cases.find(x=>x.fixture==="text-layer");
  assert.ok(text.psd_tools_logical_layers.some(x=>x.kind==="type"));
});
test("final M4c-7e outcome stays research-only regardless of preview or group-rename successes",()=>{
  assert.equal(DECISION.decision,"PSD_PRODUCTION_MUTATION_BLOCKED");
  assert.equal(DECISION.public_psd_write_authorized,false);
  assert.equal(DECISION.main_v01_release_baseline_changed,false);
  assert.equal(DECISION.evidence.photoshop_native_group_rename_save_reopen_verified_for_exact_fixture,true);
  assert.equal(DECISION.evidence.photoshop_native_composite_unchanged_in_two_isolated_group_fixture_runs,true);
  assert.equal(DECISION.evidence.image_resource_1050_slices_name_only_difference_explained_for_fixture,true);
  assert.equal(DECISION.evidence.exif_xmp_shmd_full_metadata_semantics_verified,false);
  assert.equal(DECISION.evidence.full_unknown_resource_preservation_proven,false);
  assert.equal(DECISION.evidence.ordinary_pixel_layer_mutation_in_photoshop_verified,false);
  assert.equal(DECISION.expanded_read_only_sample_review.valid_psd_psb_structures,13);
  assert.equal(DECISION.expanded_read_only_sample_review.additional_photoshop_native_read_only_cases,3);
  assert.equal(DECISION.expanded_read_only_sample_review.new_research_fixtures_promoted_to_m4c2_allowlist,0);
  assert.equal(DECISION.expanded_read_only_sample_review.new_public_writer_capabilities,0);
});
