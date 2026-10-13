"use strict";

// M4c-7d: research-only PSD metadata structure comparison. Do not normalize
// unknown blocks into a production allow-list; all raw differences remain
// unresolved until Photoshop semantics and provenance are independently proven.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {scan}=require("./m4c3_raw_block_inventory.cjs");
const ROOT=path.resolve(__dirname,"../../../../");
const BASE=path.join(ROOT,"target/m4c7-psd-photoshop-save");
const REPORT=path.join(ROOT,"docs/data/m4c7d-metadata-forensics-v1.json");
const RESOURCE_NAMES={
  1050:"Slices",1058:"EXIF_data_1",1060:"XMP_metadata"
};
const sha=b=>crypto.createHash("sha256").update(b).digest("hex");

function resourcePayload(buf,inventory,id) {
  const records=inventory.image_resources.filter(r=>r.id===id);
  if(records.length!==1)throw Error("missing or repeated resource "+id);
  const r=records[0];
  const pascalBytes=Math.ceil((1+Buffer.byteLength(r.name,"latin1"))/2)*2;
  const start=r.offset+4+2+pascalBytes+4,end=start+r.length;
  if(end>buf.length)throw Error("resource payload exceeds source");
  return buf.subarray(start,end);
}
function slicesName(data) {
  // Only parse Photoshop Slices v6: version + rectangle + 32-bit
  // UTF-16BE code-unit-count and document name. Other formats fail closed.
  if(data.length<24||data.readUInt32BE(0)!==6)throw Error("unsupported Slices layout");
  const count=data.readUInt32BE(20),start=24,end=start+count*2;
  if(count<1||count>255||end>data.length)throw Error("invalid slice document name");
  const name=Buffer.from(data.subarray(start,end)).swap16().toString("utf16le");
  return {name,units:count,prefix:data.subarray(0,20),tail:data.subarray(end)};
}
function compareSlices(a,b) {
  const x=slicesName(a),y=slicesName(b);
  return {resource_id:1050,description:RESOURCE_NAMES[1050],
    encoded_units:[x.units,y.units],names:[x.name,y.name],
    payload_lengths:[a.length,b.length],
    expected_byte_delta:(x.units-y.units)*2,
    observed_byte_delta:a.length-b.length,
    identical_except_name:
      x.prefix.equals(y.prefix)&&x.tail.equals(y.tail),
    interpretation:"file-name-like Slices document name; no generic metadata preservation waiver"
  };
}
function compareRaw(a,b) {
  const count=Math.min(a.length,b.length);
  let changed=0,first=-1,last=-1;
  for(let i=0;i<count;i++)if(a[i]!==b[i]){
    changed++;if(first===-1)first=i;last=i;
  }
  return {lengths:[a.length,b.length],
    same_length:a.length===b.length,
    changed_byte_positions_within_shared_length:changed,
    first_change_offset:first,last_change_offset:last,
    byte_equal:a.equals(b)};
}
function xmpMarkers(data) {
  const xml=data.toString("utf8");
  const keys=["CreateDate","ModifyDate","MetadataDate","DocumentID",
    "InstanceID","OriginalDocumentID","DerivedFrom","History","Ingredients"];
  return Object.fromEntries(keys.map(k=>{
    const rx=new RegExp("<[A-Za-z0-9_]+:"+k+"(?:\\s[^>]*)?>([\\s\\S]*?)<\\/[A-Za-z0-9_]+:"+k+">");
    const match=xml.match(rx);
    return [k,{present:!!match,sha256:match?sha(Buffer.from(match[1])):null}];
  }));
}
function changedXmpMarkers(a,b) {
  const x=xmpMarkers(a),y=xmpMarkers(b);
  return Object.keys(x).filter(k=>
    x[k].present!==y[k].present||x[k].sha256!==y[k].sha256);
}
function metadataItems(buf,inventory) {
  return inventory.tagged_blocks.filter(x=>x.key==="shmd").flatMap(t=>{
    const start=t.offset+8+t.length_width;
    const data=buf.subarray(start,start+t.length);
    if(data.length<4)throw Error("truncated shmd");
    const count=data.readUInt32BE(0);
    if(count>32)throw Error("unsupported shmd count");
    let pos=4;
    const items=[];
    for(let i=0;i<count;i++){
      if(pos+16>data.length)throw Error("truncated shmd entry");
      const signature=data.toString("latin1",pos,pos+4);
      const key=data.toString("latin1",pos+4,pos+8);
      const copy=data[pos+8],length=data.readUInt32BE(pos+12);
      pos+=16;
      if(length>data.length-pos)throw Error("invalid shmd payload length");
      items.push({layer_index:t.layer_index,metadata_key:key,signature,
        copy_on_sheet_duplication:copy,length,
        payload_sha256:sha(data.subarray(pos,pos+length))});
      pos+=length;
    }
    if(pos!==data.length)throw Error("unparsed shmd data remains");
    return items;
  });
}
function inspectPair(dir) {
  const absolute=path.resolve(dir);
  if(path.dirname(absolute)!==BASE||!path.basename(absolute).startsWith("run-"))
    throw Error("not an isolated Photoshop research run");
  const files=["nested-group-control-saved.psd","nested-group-candidate-saved.psd"];
  const bytes=files.map(f=>{
    const target=path.join(absolute,f);
    const info=fs.lstatSync(target);
    if(!info.isFile()||info.isSymbolicLink()||info.size>8*1024*1024)
      throw Error("unsafe saved PSD");
    return fs.readFileSync(target);
  });
  const inv=bytes.map(scan);
  const resources={};
  for(const id of [1050,1058,1060]){
    const a=resourcePayload(bytes[0],inv[0],id);
    const b=resourcePayload(bytes[1],inv[1],id);
    resources[id]={description:RESOURCE_NAMES[id],
      ...compareRaw(a,b),
      ...(id===1050?{slices_name:compareSlices(a,b)}:{}),
      ...(id===1060?{xmp_marker_keys_differing:changedXmpMarkers(a,b)}:{}),
      data_sha256_control:sha(a),data_sha256_candidate:sha(b)};
  }
  const metadata=bytes.map((b,i)=>metadataItems(b,inv[i]));
  assert.equal(metadata[0].length,metadata[1].length);
  const metadataChanges=metadata[0].map((x,i)=>({
    layer_index:x.layer_index,metadata_key:x.metadata_key,
    serialized_length:x.length,
    changed:x.payload_sha256!==metadata[1][i].payload_sha256
  }));
  return {saved_files:files.map((f,i)=>({name:f,sha256:sha(bytes[i])})),
    resources,shmd:{entries:metadataChanges.length,changed_entries:metadataChanges
      .filter(x=>x.changed).length,entries_detail:metadataChanges}};
}
function buildReport(dirs) {
  if(dirs.length!==2||dirs[0]===dirs[1])throw Error("exactly two distinct save runs");
  const runs=dirs.map(inspectPair);
  assert.ok(runs.every(x=>x.resources[1050].slices_name.identical_except_name));
  assert.ok(runs.every(x=>x.resources[1050].slices_name.observed_byte_delta===16));
  return {
    schema_version:"1",milestone:"M4c-7d-metadata",
    public_write_authorized:false,
    description:"only metadata payload type and repeatability evidence; no unknown-field waiver",
    category_status:{
      "1050":"filename-derived Slices document-name bytes explained for this exact fixture",
      "1058":"EXIF bytes may change across runs; contents not fully interpreted",
      "1060":"XMP document/history fields may change across runs; not a safe exclusion list",
      "shmd":"per-layer cust metadata changes; 52-byte payload semantic meaning unresolved"
    },
    summary:{runs:2,resource_1050_suffix_equal:runs.every(x=>
      x.resources[1050].slices_name.identical_except_name),
      resource_1050_difference_explained_for_fixture:true,
      opaque_metadata_fidelity_certified:false,production_writes_authorized:0},
    runs};
}
function check(report) {
  assert.equal(report.schema_version,"1");
  assert.equal(report.public_write_authorized,false);
  assert.equal(report.summary.runs,2);
  assert.equal(report.summary.production_writes_authorized,0);
  assert.equal(report.summary.opaque_metadata_fidelity_certified,false);
  assert.ok(report.runs.every(x=>x.resources[1050].slices_name.identical_except_name));
}
if(require.main===module){
  const mode=process.argv[2];
  if(mode==="--write" && process.argv.length===5){
    const result=buildReport(process.argv.slice(3));
    check(result);
    fs.writeFileSync(REPORT,JSON.stringify(result,null,2)+"\n");
    console.log(JSON.stringify({mode,...result.summary}));
  }else if(mode==="--check"&&process.argv.length===3){
    const result=JSON.parse(fs.readFileSync(REPORT,"utf8"));
    check(result);
    console.log(JSON.stringify({mode,offline:true,...result.summary}));
  }else{
    process.stderr.write("usage: node m4c7d_metadata_forensics.cjs --write RUN1 RUN2 | --check\n");
    process.exitCode=2;
  }
}
module.exports={resourcePayload,slicesName,compareSlices,compareRaw,xmpMarkers,
  changedXmpMarkers,metadataItems,inspectPair,buildReport,check};
