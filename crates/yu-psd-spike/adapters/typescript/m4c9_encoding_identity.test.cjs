"use strict";

const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const {
  layerEntries,strictDuplicatePatch,check:checkIdentity
}=require("./m4c9_legacy_lyid_research.cjs");
const {extractXmp,check:checkMetadata}=
  require("./m4c9_metadata_semantics.cjs");
const ROOT=path.resolve(__dirname,"../../../../");
const IDENT=require(path.join(ROOT,"docs/data/m4c9-encoding-lyid-v1.json"));
const META=require(path.join(ROOT,"docs/data/m4c9-metadata-semantics-v1.json"));
const POLICY=require(path.join(ROOT,"docs/data/m4c8-limited-rename-policy-v1.json"));
const M4C2=require(path.join(ROOT,"docs/data/psd-safe-mutation-scope-m4c2-v1.json"));

test("frozen Chinese and Cyrillic legacy bytes decode as observed GB18030",()=>{
  checkIdentity(IDENT);
  const chinese=IDENT.cases.find(x=>x.fixture==="chinese").raw_layers[0];
  const cyrillic=IDENT.cases.find(x=>x.fixture==="cyrillic").raw_layers[0];
  assert.equal(chinese.unicode_name,"测试图层");
  assert.equal(chinese.legacy_raw_hex,"b2e2cad4cdbcb2e3");
  assert.equal(chinese.legacy_as_gb18030,"测试图层");
  assert.equal(cyrillic.unicode_name,"Слой");
  assert.equal(cyrillic.legacy_raw_hex,"a7b3a7dda7e0a7db");
  assert.equal(cyrillic.legacy_as_gb18030,"Слой");
  assert.equal(chinese.legacy_encoding_certified_for_arbitrary_PSD,false);
  assert.equal(cyrillic.legacy_encoding_certified_for_arbitrary_PSD,false);
  assert.equal(IDENT.summary.language_legacy_gb18030_equals_luni,2);
});
test("a duplicate human-readable name is distinguished using lyid, not the name",()=>{
  const dup=IDENT.cases.find(x=>x.fixture==="duplicate");
  assert.deepEqual(dup.raw_layers.map(x=>x.unicode_name),["Layer 0","Layer 0"]);
  assert.deepEqual(dup.raw_layers.map(x=>x.photoshop_lyid),[6,2]);
  const e=IDENT.duplicate_identity_experiment;
  assert.equal(e.exact_stable_lyid_requested,6);
  assert.equal(e.non_target_lyid,2);
  assert.equal(e.source_layer_names_ambiguous,true);
  assert.equal(e.touched_original_bytes,2);
  assert.equal(e.native_id_binding_verified,true);
  assert.deepEqual(e.saved_raw_layer_ids,[{id:6,name:"Layer A"},{id:2,name:"Layer 0"}]);
  assert.ok(e.photoshop_saved_reopened.some(x=>x.photoshop_layer_id===6&&x.name==="Layer A"));
  assert.ok(e.photoshop_saved_reopened.some(x=>x.photoshop_layer_id===2&&x.name==="Layer 0"));
  assert.equal(e.no_write_authorization,true);
});
test("hash-mismatched or structurally unsupported PSDs must fail closed",()=>{
  const simple=fs.readFileSync(path.join(ROOT,"fixtures/psd/upstream/psd-tools/2layers.psd"));
  assert.throws(()=>layerEntries(simple),x=>x.code==="M4C9_RESEARCH_REFUSAL");
  assert.throws(()=>strictDuplicatePatch(simple,"0".repeat(64)),x=>x.code==="M4C9_RESEARCH_REFUSAL");
  assert.equal(IDENT.public_write_authorized,false);
  assert.ok(IDENT.cases.every(x=>!x.production_write_authorized));
  assert.equal(POLICY.public_psd_write_authorized,false);
  assert.equal(M4C2.public_mutation_authorized,false);
});
test("16 shmd cust layerTime payloads have identical framing but no semantic release waiver",()=>{
  checkMetadata(META);
  assert.equal(META.summary.custom_layer_time_items,16);
  assert.equal(META.summary.paired_runs,2);
  assert.equal(META.summary.all_custom_metadata_structural_prefix_equal,true);
  assert.equal(META.summary.full_EXIF_XMP_shmd_semantic_fidelity_certified,false);
  for(const r of META.runs){
    assert.equal(r.layer_time.length,4);
    assert.ok(r.layer_time.every(x=>
      x.prefix_sha_equal &&
      x.metadata_opaque_semantics_certified===false &&
      Number.isFinite(x.control_value) &&
      Number.isFinite(x.candidate_value)));
    assert.equal(r.exif.tiff_signature,"4d4d002a");
    assert.equal(r.exif.exif_content_semantics_certified,false);
    assert.equal(r.xmp.all_XMP_properties_semantically_verified,false);
  }
});
test("XMP parser locates document identity fields without asserting contents are ignorable",()=>{
  const a=extractXmp(Buffer.from('<x:xmp><xmpMM:DocumentID>A</xmpMM:DocumentID>'+
    '<xmpMM:InstanceID>Z</xmpMM:InstanceID><xmp:ModifyDate>2026</xmp:ModifyDate></x:xmp>'));
  const b=extractXmp(Buffer.from('<x:xmp><xmpMM:DocumentID>B</xmpMM:DocumentID>'+
    '<xmpMM:InstanceID>Z</xmpMM:InstanceID><xmp:ModifyDate>2026</xmp:ModifyDate></x:xmp>'));
  assert.notEqual(a.DocumentID.sha256,b.DocumentID.sha256);
  assert.equal(a.InstanceID.sha256,b.InstanceID.sha256);
  assert.equal(a.History.present,false);
  assert.equal(META.summary.production_writes_authorized,0);
});
