"use strict";

const {test}=require("node:test");
const assert=require("node:assert/strict");
const fs=require("node:fs");
const path=require("node:path");
const {utf16be}=require("./m4c5_byte_patch_spike.cjs");
const {patchSynced,CASES,check:checkNames}=
  require("./m4c10_multilingual_dual_sync.cjs");
const {check:checkIdentity}=require("./m4c10_lyid_structural_stability.cjs");
const {check:checkMetadata}=require("./m4c10_metadata_paired_audit.cjs");
const ROOT=path.resolve(__dirname,"../../../../");
const NAMES=require(path.join(ROOT,"docs/data/m4c10-multilingual-sync-v1.json"));
const ID=require(path.join(ROOT,"docs/data/m4c10-lyid-reorder-group-v1.json"));
const META=require(path.join(ROOT,"docs/data/m4c10-metadata-paired-v1.json"));
const PRIOR=require(path.join(ROOT,"docs/data/m4c9-encoding-lyid-v1.json"));

test("Photoshop native two-language UTF16 and GB18030 synced source-pinned patches reopen",()=>{
  checkNames(NAMES);
  assert.equal(NAMES.summary.paired_native_runs,2);
  assert.equal(NAMES.summary.synchronized_multilingual_names_reopened,2);
  assert.equal(NAMES.summary.metadata_fidelity_certifications,0);
  assert.equal(NAMES.summary.production_writes_authorized,0);
  for(const c of NAMES.cases){
    assert.equal(c.UTF16BE_and_GB18030_synchronized,true);
    assert.equal(c.native_name_reopen_verified,true);
    assert.equal(c.saved_lyid_and_dual_name_verified,true);
    assert.equal(c.independent_saved_composite_equal,true);
    assert.equal(c.full_opaque_metadata_semantics_certified,false);
    assert.equal(c.source_and_inputs_unchanged,true);
    assert.equal(c.public_write_authorized,false);
    assert.ok(c.changed_name_bytes>0);
    assert.match(c.source_sha256,/^[0-9a-f]{64}$/);
    assert.match(c.candidate_sha256,/^[0-9a-f]{64}$/);
    assert.notEqual(c.source_sha256,c.candidate_sha256);
    assert.ok(c.photoshop_reopened_candidate_names.some(x=>x.name===c.requested_name));
  }
});

test("four-character multilingual replacements have fixed UTF-16 and GB18030 byte widths",()=>{
  for(const c of CASES){
    assert.equal(utf16be(c.old).length,8);
    assert.equal(utf16be(c.next).length,8);
    const orig=Buffer.from(c.oldGb,"hex"),newName=Buffer.from(c.nextGb,"hex");
    assert.equal(orig.length,8);
    assert.equal(newName.length,8);
    assert.equal(new TextDecoder("gb18030",{fatal:true}).decode(orig),c.old);
    assert.equal(new TextDecoder("gb18030",{fatal:true}).decode(newName),c.next);
  }
  assert.equal(NAMES.public_write_authorized,false);
});

test("wrong source SHA, truncated or other PSD input cannot be patched",()=>{
  const bad=Buffer.from("not a Photoshop file");
  assert.throws(()=>patchSynced(bad,"0".repeat(64),CASES[0]),
    x=>x.code==="M4C10_REFUSED");
  const valid=fs.readFileSync(path.join(ROOT,
    "fixtures/psd/benchmark/upstream/psd-tools/background-red-opacity-80.psd"));
  assert.throws(()=>patchSynced(valid,"f".repeat(64),CASES[0]),
    x=>x.code==="M4C10_REFUSED");
});

test("native layer IDs 6 and 2 survive reorder and new Photoshop group nesting",()=>{
  checkIdentity(ID);
  assert.equal(ID.summary.native_structural_operations_tested,2);
  assert.equal(ID.summary.operations_preserving_target_lyids,2);
  assert.equal(ID.summary.production_writes_authorized,0);
  const reordered=ID.cases.find(x=>x.action==="reorder");
  const grouped=ID.cases.find(x=>x.action==="group");
  assert.deepEqual(reordered.native_layers.map(x=>x.photoshop_lyid),[6,2]);
  assert.ok(reordered.raw_lyid_records.find(x=>x.photoshop_lyid===6).raw_index!==
    PRIOR.cases.find(x=>x.fixture==="duplicate").raw_layers.find(x=>
      x.photoshop_lyid===6).raw_index);
  const original=grouped.native_layers.find(x=>x.photoshop_lyid===6);
  const sibling=grouped.native_layers.find(x=>x.photoshop_lyid===2);
  assert.equal(original.depth,1);
  assert.equal(sibling.depth,0);
  assert.ok(grouped.native_layers.some(x=>x.group&&x.name==="M4c10 Research Group"));
  assert.ok(grouped.raw_lyid_records.some(x=>x.photoshop_lyid===6));
  assert.ok(grouped.raw_lyid_records.some(x=>x.photoshop_lyid===2));
  assert.ok(ID.cases.every(x=>x.source_and_input_unchanged && !x.public_write_authorized));
});

test("saved multilingual pairs retain channels while still failing opaque metadata certification",()=>{
  checkMetadata(META);
  assert.equal(META.summary.paired_native_multilingual_cases,2);
  assert.equal(META.summary.pairs_with_identical_raw_channels,2);
  assert.equal(META.summary.pairs_with_identical_compressed_merged_image,2);
  assert.equal(META.summary.pairs_with_unresolved_metadata,2);
  assert.equal(META.summary.production_writes_authorized,0);
  for(const c of META.cases){
    assert.deepEqual(c.raw_channel_delta,{added:[],removed:[],changed:[]});
    assert.equal(c.compressed_merged_image_equal,true);
    assert.equal(c.unknown_metadata_preservation_proven,false);
    assert.equal(c.production_write_authorized,false);
    assert.ok(c.raw_image_resource_changes_nonzero || c.tags.changed.length>0);
  }
});
