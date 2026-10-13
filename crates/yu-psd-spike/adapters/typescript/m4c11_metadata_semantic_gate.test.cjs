"use strict";

const {test}=require("node:test");
const assert=require("node:assert/strict");
const path=require("node:path");
const {inspectTiff,compareTiff,check}=
  require("./m4c11_metadata_semantic_gate.cjs");
const ROOT=path.resolve(__dirname,"../../../../");
const EVIDENCE=require(path.join(ROOT,
  "docs/data/m4c11-metadata-semantic-gate-v1.json"));
const OUTPUT=require(path.join(ROOT,"docs/data/m4c11-safe-output-v1.json"));

function tiffFixture(date){
  if(Buffer.byteLength(date,"ascii")!==20)throw Error("20-byte TIFF DateTime required");
  const b=Buffer.alloc(46);
  b.write("II",0,"ascii");
  b.writeUInt16LE(42,2);
  b.writeUInt32LE(8,4);
  b.writeUInt16LE(1,8);    // one IFD entry
  b.writeUInt16LE(306,10); // TIFF DateTime
  b.writeUInt16LE(2,12);   // ASCII
  b.writeUInt32LE(20,14);
  b.writeUInt32LE(26,18);  // payload offset
  b.writeUInt32LE(0,22);   // no next IFD
  b.write(date,26,"ascii");
  return b;
}
test("two exact Photoshop multilingual saved pairs classify real EXIF DateTime changes",()=>{
  check(EVIDENCE);
  assert.equal(EVIDENCE.summary.paired_multilingual_cases,2);
  assert.equal(EVIDENCE.summary.cases_with_XMP_identity_or_history_deltas,2);
  assert.equal(EVIDENCE.summary.cases_with_EXIF_tag_or_byte_differences,2);
  assert.equal(EVIDENCE.summary.cases_with_unknown_semantics,2);
  assert.equal(EVIDENCE.public_write_authorized,false);
  for(const x of EVIDENCE.cases){
    const exif=x.EXIF_tag_forensics;
    assert.deepEqual(exif.total_tags,[22,22]);
    assert.deepEqual(exif.ifd_count,[3,3]);
    assert.deepEqual(exif.changed_known_fields,["DateTime"]);
    assert.deepEqual(exif.changed_tag_keys,["8:306:2:20"]);
    assert.deepEqual(exif.removed_tag_keys,[]);
    assert.deepEqual(exif.added_tag_keys,[]);
    assert.equal(exif.complete_semantic_preservation_certified,false);
    assert.equal(x.Slices_name_forensics.identical_except_name,true);
    assert.equal(x.shmd.raw_entries_equal,true);
    assert.equal(x.shmd.complete_semantics_proven,false);
    assert.equal(x.compressed_merged_image_equal,true);
    assert.deepEqual(x.channel_delta,{added:[],removed:[],changed:[]});
    assert.equal(x.all_opaque_metadata_semantics_proven,false);
    assert.equal(x.production_write_authorized,false);
  }
});
test("XMP field identity/provenance never silently ignored",()=>{
  for(const x of EVIDENCE.cases){
    assert.ok(x.XMP_unproven_semantic_fields.includes("DocumentID"));
    assert.ok(x.XMP_unproven_semantic_fields.includes("InstanceID"));
    assert.ok(x.XMP_unproven_semantic_fields.includes("History"));
    assert.ok(x.XMP_fields.some(y=>y.field==="ModifyDate"&&!y.sha256_equal));
    assert.ok(x.XMP_fields.every(y=>y.metadata_identity_field_not_ignored));
    assert.equal(x.resource_delta.changed.length>0,true);
  }
  assert.equal(EVIDENCE.exclusion_policy.startsWith("NONE"),true);
  assert.equal(EVIDENCE.summary.public_writes_authorized,0);
});
test("bounded TIFF parser identifies one ASCII DateTime field without certifying semantics",()=>{
  const a=tiffFixture("2026:10:13 09:30:00\0");
  const b=tiffFixture("2026:10:13 10:30:00\0");
  const parsed=inspectTiff(a);
  assert.equal(parsed.byte_order,"II");
  assert.equal(parsed.ifd_count,1);
  assert.equal(parsed.entries.length,1);
  assert.deepEqual(parsed.entries[0].known_field_name,"DateTime");
  assert.equal(parsed.completely_recognized_exif_semantics,false);
  const comparison=compareTiff(a,b);
  assert.deepEqual(comparison.changed_known_fields,["DateTime"]);
  assert.equal(comparison.equal_raw_bytes,false);
  assert.equal(comparison.complete_semantic_preservation_certified,false);
});
test("TIFF malformed offsets, unknown entry types and loops fail closed",()=>{
  const example=tiffFixture("2026:10:13 09:30:00\0");
  const badMagic=Buffer.from(example);
  badMagic.writeUInt16LE(123,2);
  assert.throws(()=>inspectTiff(badMagic),/TIFF magic/);
  const outOfRange=Buffer.from(example);
  outOfRange.writeUInt32LE(0xfffffff0,18);
  assert.throws(()=>inspectTiff(outOfRange),/outside buffer/);
  const unknownType=Buffer.from(example);
  unknownType.writeUInt16LE(65535,12);
  assert.throws(()=>inspectTiff(unknownType),/unsupported TIFF data type/);
  const cycle=Buffer.from(example);
  cycle.writeUInt32LE(8,22);
  assert.throws(()=>inspectTiff(cycle),/cyclic/);
});
test("safe-output snapshot never upgrades research bytes to production PSD writing",()=>{
  assert.equal(OUTPUT.milestone,"M4c-11");
  assert.equal(OUTPUT.public_write_authorized,false);
  assert.equal(OUTPUT.summary.private_staged_published,1);
  assert.equal(OUTPUT.summary.production_writes_authorized,0);
  const x=OUTPUT.cases[0];
  assert.equal(x.same_filesystem_atomic_hard_link,true);
  assert.equal(x.output_exists_verified,true);
  assert.equal(x.source_version_rechecked,true);
  assert.equal(x.full_opaque_metadata_fidelity_certified,false);
  assert.equal(x.native_photoshop_save_not_performed,true);
  assert.equal(x.public_write_authorized,false);
});
