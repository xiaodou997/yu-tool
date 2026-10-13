"use strict";

// Bounded, read-only forensic evidence; never normalize or strip real PSD
// metadata. In particular, XMP identity/history and unknown EXIF/shmd
// fields must NOT become a permissive exclusion list for a writer.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {scan}=require("./m4c3_raw_block_inventory.cjs");
const {resourcePayload,compareSlices,metadataItems}=
  require("./m4c7d_metadata_forensics.cjs");
const {extractXmp}=require("./m4c9_metadata_semantics.cjs");
const {rawDelta}=require("./m4c7d_photoshop_delta_audit.cjs");
const BASE_REPORT=require("../../../../docs/data/m4c10-multilingual-sync-v1.json");
const ROOT=path.resolve(__dirname,"../../../../");
const REPORT=path.join(ROOT,"docs/data/m4c11-metadata-semantic-gate-v1.json");
const sha=x=>crypto.createHash("sha256").update(x).digest("hex");
const TYPE_SIZES={1:1,2:1,3:2,4:4,5:8,6:1,7:1,8:2,9:4,10:8,11:4,12:8};
const KNOWN_TIFF_TAGS={306:"DateTime",0x8769:"ExifIFDPointer",
  0x8825:"GPSIFDPointer",0xa005:"InteroperabilityIFDPointer"};

function inspectTiff(tiff) {
  if(!Buffer.isBuffer(tiff)||tiff.length<8||tiff.length>1_000_000)
    throw Error("TIFF EXIF payload invalid");
  const marker=tiff.toString("latin1",0,2);
  if(!["II","MM"].includes(marker))throw Error("unknown EXIF byte order");
  const little=marker==="II";
  const u16=i=>little?tiff.readUInt16LE(i):tiff.readUInt16BE(i);
  const u32=i=>little?tiff.readUInt32LE(i):tiff.readUInt32BE(i);
  if(u16(2)!==42)throw Error("invalid TIFF magic");
  const ifdOffsets=[u32(4)],seen=new Set(),entries=[];
  while(ifdOffsets.length>0){
    const base=ifdOffsets.shift();
    if(base===0)continue;
    if(seen.has(base)||seen.size>=8)throw Error("cyclic or over-budget TIFF IFD");
    seen.add(base);
    if(base+2>tiff.length)throw Error("IFD header out of bounds");
    const count=u16(base);
    if(count>128||base+2+12*count+4>tiff.length)
      throw Error("unsupported EXIF IFD size");
    for(let index=0;index<count;index++){
      const at=base+2+12*index;
      const tag=u16(at),type=u16(at+2),amount=u32(at+4);
      if(!TYPE_SIZES[type])throw Error("unsupported TIFF data type");
      const length=amount*TYPE_SIZES[type];
      if(!Number.isSafeInteger(length)||length>65_536)
        throw Error("unsupported EXIF field length");
      const position=length<=4?at+8:u32(at+8);
      if(position+length>tiff.length)throw Error("EXIF field outside buffer");
      const raw=tiff.subarray(position,position+length);
      entries.push({ifd_offset:base,tag,
        known_field_name:KNOWN_TIFF_TAGS[tag]||null,
        type,count:amount,bytes:length,sha256:sha(raw)});
      // Descend into the usual EXIF and GPS IFD pointers only.
      if((tag===0x8769||tag===0x8825||tag===0xa005)&&length===4) {
        ifdOffsets.push(u32(position));
      }
    }
    const next=u32(base+2+12*count);
    if(next!==0)ifdOffsets.push(next);
  }
  return {byte_order:marker,ifd_count:seen.size,entries,
    completely_recognized_exif_semantics:false};
}
function compareTiff(a,b){
  const x=inspectTiff(a),y=inspectTiff(b);
  const key=e=>e.ifd_offset+":"+e.tag+":"+e.type+":"+e.count;
  const toMap=v=>new Map(v.entries.map(e=>[key(e),e.sha256]));
  const first=toMap(x),second=toMap(y);
  const changed=[],removed=[],added=[];
  for(const [k,v] of first){
    if(!second.has(k))removed.push(k);
    else if(second.get(k)!==v)changed.push(k);
  }
  for(const k of second.keys())if(!first.has(k))added.push(k);
  return {byte_order:[x.byte_order,y.byte_order],ifd_count:[x.ifd_count,y.ifd_count],
    total_tags:[x.entries.length,y.entries.length],
    changed_tag_keys:changed,removed_tag_keys:removed,added_tag_keys:added,
    changed_known_fields:x.entries.filter(e=>changed.includes(key(e))&&
      e.known_field_name).map(e=>e.known_field_name),
    equal_raw_bytes:a.equals(b),
    complete_semantic_preservation_certified:false};
}
function snapshotFromRun(dir) {
  const d=path.resolve(dir);
  if(!path.basename(d).startsWith("run-")||
    path.basename(path.dirname(d))!=="m4c10-multilingual-sync")
    throw Error("only M4c-10 isolated saved control/candidate run permitted");
  return BASE_REPORT.cases.map(c=>{
    const names=[c.fixture+"-original-saved.psd",c.fixture+"-synced-saved.psd"];
    const bodies=names.map((name,i)=>{
      const f=path.join(d,name),st=fs.lstatSync(f);
      if(!st.isFile()||st.isSymbolicLink()||st.size>8*1024*1024)
        throw Error("unsafe saved experiment PSD");
      const bytes=fs.readFileSync(f);
      const expected=i===0?c.saved_original.sha256:c.saved_candidate.sha256;
      if(sha(bytes)!==expected)throw Error("M4c-10 SHA mismatch");
      return bytes;
    });
    const inv=bodies.map(scan);
    const XMP=bodies.map((bytes,i)=>extractXmp(resourcePayload(bytes,inv[i],1060)));
    const XMPFields=Object.keys(XMP[0]).map(key=>({
      field:key,present_in_control:XMP[0][key].present,
      present_in_candidate:XMP[1][key].present,
      sha256_equal:XMP[0][key].sha256===XMP[1][key].sha256,
      metadata_identity_field_not_ignored:true
    }));
    const EXIF=compareTiff(...bodies.map((b,i)=>resourcePayload(b,inv[i],1058)));
    const slices=compareSlices(...bodies.map((b,i)=>resourcePayload(b,inv[i],1050)));
    const shmd=bodies.map((b,i)=>metadataItems(b,inv[i]));
    const tagList=(items)=>items.map(x=>({
      layer_index:x.layer_index,key:x.metadata_key,
      signature:x.signature,length:x.length,payload_sha256:x.payload_sha256
    }));
    const metaEqual=JSON.stringify(tagList(shmd[0]))===JSON.stringify(tagList(shmd[1]));
    const delta=rawDelta(bodies[0],bodies[1]);
    return {
      fixture:c.fixture,
      SHA_bound_input_pair:[c.saved_original.sha256,c.saved_candidate.sha256],
      XMP_fields:XMPFields,
      XMP_unproven_semantic_fields:XMPFields.filter(x=>!x.sha256_equal).map(x=>x.field),
      EXIF_tag_forensics:EXIF,
      Slices_name_forensics:{
        names:slices.names,identical_except_name:slices.identical_except_name,
        observed_byte_delta:slices.observed_byte_delta,
        expected_byte_delta:slices.expected_byte_delta
      },
      shmd:{number_control:shmd[0].length,number_candidate:shmd[1].length,
        raw_entries_equal:metaEqual,
        entry_types:[...new Set(shmd[0].map(x=>x.metadata_key))],
        complete_semantics_proven:false
      },
      resource_delta:delta.image_resources,
      tagged_block_delta:delta.additional_layer_information,
      channel_delta:delta.channel_pixels,
      compressed_merged_image_equal:delta.compressed_composite_equal,
      all_opaque_metadata_semantics_proven:false,
      production_write_authorized:false
    };
  });
}
function build(dir){
  const cases=snapshotFromRun(dir);
  assert.equal(cases.length,2);
  return {
    schema_version:"1",milestone:"M4c-11",
    operation:"pinned Photoshop save EXIF TIFF XMP identity shmd forensic gate",
    public_write_authorized:false,
    summary:{
      paired_multilingual_cases:cases.length,
      cases_with_XMP_identity_or_history_deltas:cases.filter(x=>
        x.XMP_unproven_semantic_fields.some(y=>
          ["DocumentID","InstanceID","History"].includes(y))).length,
      cases_with_EXIF_tag_or_byte_differences:cases.filter(x=>
        !x.EXIF_tag_forensics.equal_raw_bytes).length,
      cases_with_unknown_semantics:cases.filter(x=>
        !x.all_opaque_metadata_semantics_proven).length,
      public_writes_authorized:0
    },
    exclusion_policy:"NONE: recognize fields but never waive unknown field/content differences",
    cases
  };
}
function check(report){
  assert.equal(report.milestone,"M4c-11");
  assert.equal(report.public_write_authorized,false);
  assert.equal(report.summary.paired_multilingual_cases,2);
  assert.equal(report.summary.cases_with_unknown_semantics,2);
  assert.equal(report.summary.public_writes_authorized,0);
  assert.ok(report.cases.every(x=>!x.all_opaque_metadata_semantics_proven &&
    !x.production_write_authorized));
}
if(require.main===module){
  if(process.argv[2]==="--write"&&process.argv.length===4){
    const report=build(process.argv[3]);check(report);
    fs.writeFileSync(REPORT,JSON.stringify(report,null,2)+"\n");
    console.log(JSON.stringify({mode:"--write",...report.summary}));
  }else if(process.argv[2]==="--check"&&process.argv.length===3){
    const r=JSON.parse(fs.readFileSync(REPORT,"utf8"));check(r);
    console.log(JSON.stringify({mode:"--check",offline:true,...r.summary}));
  }else{
    process.stderr.write("usage: node m4c11_metadata_semantic_gate.cjs --write M4C10_RUN | --check\n");
    process.exitCode=2;
  }
}
module.exports={inspectTiff,compareTiff,build,check};
