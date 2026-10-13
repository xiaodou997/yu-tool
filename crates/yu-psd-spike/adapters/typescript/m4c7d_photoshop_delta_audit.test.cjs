"use strict";

const assert=require("node:assert/strict");
const {test}=require("node:test");
const path=require("node:path");
const fs=require("node:fs");
const {checkSnapshot}=require("./m4c7d_photoshop_delta_audit.cjs");
const {compareRaw,compareSlices,slicesName,check:checkMetadata}=
  require("./m4c7d_metadata_forensics.cjs");

const ROOT=path.resolve(__dirname,"../../../../");
const PREVIEW=require(path.join(ROOT,
  "docs/data/m4c7d-photoshop-metadata-preview-v1.json"));
const META=require(path.join(ROOT,"docs/data/m4c7d-metadata-forensics-v1.json"));

test("two independent Photoshop saves show same 160-pixel independent-parser discrepancy",()=>{
  checkSnapshot(PREVIEW);
  assert.equal(PREVIEW.summary.repeated_native_save_runs,2);
  assert.equal(PREVIEW.public_write_authorized,false);
  assert.equal(PREVIEW.summary.source_preview_changed_pixels,160);
  assert.equal(PREVIEW.summary.edited_input_preview_changed_pixels,160);
  assert.equal(PREVIEW.summary.exact_saved_pair_bytes_only_name_changed,false);
  assert.equal(PREVIEW.summary.production_writes_authorized,0);
  for(const r of PREVIEW.runs){
    assert.equal(r.preview.source_vs_control_saved.changed_pixels,160);
    assert.equal(r.preview.patched_input_vs_candidate_saved.changed_pixels,160);
    assert.deepEqual(r.preview.source_vs_control_saved.changed_bbox_xyxy,[26,25,65,97]);
    assert.deepEqual(r.preview.source_vs_control_saved.changed_channel_values,[160,160,160,0]);
    assert.equal(r.preview.source_vs_control_saved.max_channel_delta,11);
    assert.equal(r.preview.control_saved_vs_candidate_saved.changed_pixels,0);
  }
});

test("native Photoshop PNGs show zero changed pixels across source/save/rename",()=>{
  for(const r of PREVIEW.runs){
    for(const key of [
      "native_source_vs_native_saved_control",
      "native_patch_vs_native_saved_candidate",
      "native_saved_control_vs_native_saved_candidate"]){
      assert.equal(r.native_rendering[key].changed_pixels,0);
    }
    const x=r.native_rendering.independent_parser_vs_native_for_each_file;
    assert.equal(x.source.changed_pixels,160);
    assert.equal(x.patched_input.changed_pixels,160);
    assert.equal(x.control_saved.changed_pixels,0);
    assert.equal(x.candidate_saved.changed_pixels,0);
  }
  assert.equal(PREVIEW.classification.photoshop_visual_equivalence_independently_certified,false);
  assert.equal(PREVIEW.classification.comprehensive_metadata_fidelity_certified,false);
});

test("metadata types and Photoshop Slices name are identified without allowing mutation",()=>{
  checkMetadata(META);
  assert.equal(META.summary.runs,2);
  assert.equal(META.summary.opaque_metadata_fidelity_certified,false);
  assert.equal(META.public_write_authorized,false);
  for(const r of META.runs){
    const name=r.resources["1050"].slices_name;
    assert.deepEqual(name.names,["nested-group-control","nested-group"]);
    assert.equal(name.identical_except_name,true);
    assert.equal(name.observed_byte_delta,16);
    assert.equal(name.expected_byte_delta,16);
    assert.equal(r.resources["1058"].description,"EXIF_data_1");
    assert.equal(r.resources["1060"].description,"XMP_metadata");
    assert.ok(r.resources["1060"].xmp_marker_keys_differing.includes("DocumentID"));
    assert.equal(r.shmd.entries,4);
    assert.equal(r.shmd.changed_entries,4);
    assert.ok(r.shmd.entries_detail.every(x=>x.metadata_key==="cust"&&x.serialized_length===52));
  }
});

test("varying Photoshop metadata makes raw saved-pair fidelity remain blocked",()=>{
  assert.ok(PREVIEW.between_runs.unchanged_control_across_runs
    .image_resources.changed.includes("1060:#0"));
  assert.ok(PREVIEW.between_runs.unchanged_control_across_runs
    .additional_layer_information.changed.some(x=>x.endsWith(":shmd#0")));
  assert.ok(PREVIEW.runs.every(r=>r.native_saved_pair_strict_check.strict_saved_pair_bytes_passed===false));
  assert.equal(PREVIEW.summary.exact_saved_pair_bytes_only_name_changed,false);
});

test("Slices decoder refuses unknown layouts, while complete name replacement preserves remaining bytes",()=>{
  assert.throws(()=>slicesName(Buffer.alloc(8)),/unsupported Slices/);
  const invalid=Buffer.alloc(30);invalid.writeUInt32BE(6,0);
  invalid.writeUInt32BE(9999,20);
  assert.throws(()=>slicesName(invalid),/invalid slice document name/);
  const prefix=Buffer.alloc(20);prefix.writeUInt32BE(6,0);
  const suffix=Buffer.from("example-tail");
  const encoded=s=>Buffer.from(s,"utf16le").swap16();
  const pack=s=>{
    const len=Buffer.alloc(4);len.writeUInt32BE(s.length,0);
    return Buffer.concat([prefix,len,encoded(s),suffix]);
  };
  assert.equal(compareSlices(pack("control"),pack("renamed")).identical_except_name,true);
  assert.equal(compareSlices(pack("control"),pack("renamed")).observed_byte_delta,0);
  const altered=pack("renamed");altered[altered.length-1]^=1;
  assert.equal(compareSlices(pack("control"),altered).identical_except_name,false);
  assert.equal(compareRaw(Buffer.from([1,2]),Buffer.from([1,3])).changed_byte_positions_within_shared_length,1);
});
