"use strict";

const {test}=require("node:test");
const assert=require("node:assert/strict");
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const {scan}=require("./m4c3_raw_block_inventory.cjs");
const {decodeLuni}=require("./m4c5_byte_patch_spike.cjs");
const {patchKnownPixel,ORIGINAL_SHA}=require("./m4c8_pixel_rename_research.cjs");
const {check:checkNative,rawNames}=require("./m4c8_multilingual_native_probe.cjs");
const {check:checkLuni,patchLuniOnly}=
  require("./m4c8_luni_legacy_divergence.cjs");

const ROOT=path.resolve(__dirname,"../../../../");
const input=path.join(ROOT,
  "fixtures/psd/benchmark/upstream/psd-tools/background-red-opacity-80.psd");
const sha=b=>crypto.createHash("sha256").update(b).digest("hex");
const PIXEL=require(path.join(ROOT,"docs/data/m4c8-pixel-photoshop-fidelity-v1.json"));
const NAMES=require(path.join(ROOT,"docs/data/m4c8-multilingual-photoshop-names-v1.json"));
const LUNI=require(path.join(ROOT,"docs/data/m4c8-luni-only-multilingual-v1.json"));
const POLICY=require(path.join(ROOT,"docs/data/m4c8-limited-rename-policy-v1.json"));

test("exact-hash pinned normal RGB pixel changes only Unicode and ASCII Pascal name bytes",()=>{
  const source=fs.readFileSync(input);
  assert.equal(sha(source),ORIGINAL_SHA);
  const c=patchKnownPixel(source);
  assert.notEqual(c.sha256,ORIGINAL_SHA);
  assert.equal(source.length,c.bytes.length);
  assert.equal(c.changed_bytes,2);
  for(let i=0;i<source.length;i++)if(source[i]!==c.bytes[i])
    assert.ok(c.touched_ranges.some(x=>i>=x.start&&i<x.end),
      "unrelated PSD source byte modified");
  const before=scan(source),after=scan(c.bytes);
  assert.deepEqual(before.image_resources,after.image_resources);
  assert.deepEqual(before.merged_image,after.merged_image);
  assert.deepEqual(before.layer_records.map(r=>r.channels),
    after.layer_records.map(r=>r.channels));
  assert.equal(after.layer_records[0].legacy_name,"Layer A");
  const tagged=after.tagged_blocks.find(x=>x.scope==="layer"&&
    x.layer_index===0&&x.key==="luni");
  assert.equal(decodeLuni(c.bytes,tagged).value,"Layer A");
  assert.equal(sha(fs.readFileSync(input)),ORIGINAL_SHA);
});

test("unrecognized source content can never be patched by research engine",()=>{
  const bytes=Buffer.from(fs.readFileSync(input));
  bytes[bytes.length-1]^=1;
  assert.throws(()=>patchKnownPixel(bytes),x=>
    x.code==="RESEARCH_ONLY_REFUSAL");
  assert.equal(POLICY.normal_pixel_study.source_sha256,ORIGINAL_SHA);
  assert.equal(POLICY.public_psd_write_authorized,false);
  assert.equal(POLICY.production_psd_rename_available,false);
});

test("Photoshop saved/reopened pixel layer name and independent saved composite match",()=>{
  assert.equal(PIXEL.public_write_authorized,false);
  assert.equal(PIXEL.summary.photoshop_save_pairs,1);
  assert.equal(PIXEL.summary.native_pixel_name_reopen_verified,1);
  assert.equal(PIXEL.summary.strict_opaque_metadata_fidelity_certified,0);
  const c=PIXEL.cases[0];
  assert.equal(c.status,"pixel_name_reopen_verified_only_not_writer_approval");
  assert.equal(c.byte_patch_changed_bytes,2);
  assert.equal(c.native_input_names_changed_only_target,true);
  assert.equal(c.native_reopen_persists_only_target_name,true);
  assert.equal(c.independent_saved_pixel_equal,true);
  assert.equal(c.independent_saved_tree_only_name_changed,true);
  assert.equal(c.all_original_and_experiment_inputs_unchanged,true);
  assert.equal(c.saved_raw_pair.full_strict_byte_gate_passed,false);
  assert.deepEqual(c.saved_raw_pair.channels,{added:[],removed:[],changed:[]});
  assert.equal(c.saved_raw_pair.merged_composite_equal,true);
  assert.equal(c.public_write_authorized,false);
});

test("Photoshop generated Chinese, Cyrillic and duplicate layers remain research-only",()=>{
  checkNative(NAMES);
  assert.equal(NAMES.summary.scenarios,3);
  assert.equal(NAMES.summary.saved_and_reopened,3);
  assert.equal(NAMES.summary.legacy_unicode_divergent,2);
  assert.equal(NAMES.summary.production_write_authorizations,0);
  for(const c of NAMES.cases){
    assert.equal(c.status,"native_name_observation_passed");
    assert.equal(c.native_name_after_reopen_correct,true);
    assert.equal(c.independent_unicode_name_correct,true);
    assert.equal(c.original_and_research_input_unchanged,true);
    assert.equal(c.public_write_authorized,false);
    assert.match(c.output_sha256,/^[a-f\d]{64}$/);
  }
  assert.equal(NAMES.cases.find(x=>x.scenario==="chinese").has_dual_name_difference,true);
  assert.equal(NAMES.cases.find(x=>x.scenario==="cyrillic").has_dual_name_difference,true);
  const duplicates=NAMES.cases.find(x=>x.scenario==="duplicate");
  assert.equal(duplicates.duplicate_names_visible,true);
  assert.equal(duplicates.native_layers.filter(x=>x.name==="Layer 0").length,2);
});

test("luni-only name edits are visible in Photoshop but leave unvalidated legacy bytes",()=>{
  checkLuni(LUNI);
  assert.equal(LUNI.summary.tested,2);
  assert.equal(LUNI.summary.photoshop_unicode_name_read_matches,2);
  assert.equal(LUNI.summary.unresolved_legacy_mismatch,2);
  for(const c of LUNI.cases){
    assert.equal(c.unicode_patch_native_reopen_shows_new_name,true);
    assert.equal(c.unicode_patch_psd_tools_shows_new_name,true);
    assert.equal(c.legacy_pascal_untouched,true);
    assert.equal(c.status,"research_only_legacy_divergent");
    assert.equal(c.public_write_authorized,false);
  }
  const fake=fs.readFileSync(input);
  assert.throws(()=>patchLuniOnly(fake,ORIGINAL_SHA,"Слой","Тест"),
    /Unicode name|source hash|unsupported/);
  assert.ok(POLICY.blocked.includes("unicode_only_patch_with_stale_legacy_pascal"));
  assert.ok(POLICY.blocked.includes("native_Photoshop_Background"));
});
