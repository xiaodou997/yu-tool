"use strict";

// M4c-9 scoped forensic decoder for Photoshop-saved experimental metadata.
// Distinguishes identified date-like layerTime and XMP identity fields from
// semantic preservation certification. Strictly READ ONLY.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {scan}=require("./m4c3_raw_block_inventory.cjs");
const HISTORY=require(path.resolve(__dirname,
  "../../../../docs/data/m4c7d-photoshop-metadata-preview-v1.json"));
const ROOT=path.resolve(__dirname,"../../../../");
const OUTPUT=path.join(ROOT,"docs/data/m4c9-metadata-semantics-v1.json");
const SHA=b=>crypto.createHash("sha256").update(b).digest("hex");

function resourceData(bytes,inv,id){
  const found=inv.image_resources.filter(x=>x.id===id);
  if(found.length!==1)throw Error("unexpected count of image resource "+id);
  const r=found[0],encodedName=Math.ceil((1+r.name.length)/2)*2;
  const start=r.offset+4+2+encodedName+4;
  if(start+r.length>bytes.length)throw Error("resource bounds");
  return bytes.subarray(start,start+r.length);
}
function layerTimeFields(bytes, inv) {
  const result=[];
  for(const tag of inv.tagged_blocks.filter(x=>x.scope==="layer"&&x.key==="shmd")){
    const offset=tag.offset+8+tag.length_width;
    const raw=bytes.subarray(offset,offset+tag.length);
    if(raw.length!==72||raw.readUInt32BE(0)!==1)throw Error("unsupported metadata count");
    if(raw.toString("ascii",4,8)!=="8BIM" ||
      raw.toString("ascii",8,12)!=="cust" ||
      raw.readUInt32BE(16)!==52)throw Error("unexpected shmd/cust framing");
    const data=raw.subarray(20,72);
    const head="00000010000000010000000000086d6574616461746100000001000000096c6179657254696d65646f7562";
    if(data.subarray(0,43).toString("hex")!==head || data[51]!==0)
      throw Error("unrecognized custom metadata structure");
    const value=data.readDoubleBE(43);
    if(!Number.isFinite(value)||value<1e8||value>1e11)
      throw Error("unexpected layerTime numeric value");
    result.push({
      raw_layer_index:tag.layer_index,
      metadata_key:"cust",
      descriptor_field:"layerTime",
      declared_type:"doub",
      timestamp_like_float64:value,
      declared_frame_bytes:52,
      fixed_prefix_sha256:SHA(data.subarray(0,43)),
      trailing_zero_byte:true,
      semantics_certified_for_arbitrary_files:false
    });
  }
  if(result.length!==4)throw Error("unexpected native group layer metadata");
  return result;
}
function extractXmp(data){
  const xml=data.toString("utf8");
  const result={};
  for(const name of ["DocumentID","InstanceID","OriginalDocumentID",
    "ModifyDate","MetadataDate","History"]){
    const regex=new RegExp("<[A-Za-z][\\w]*:"+name+"(?:\\s[^>]*)?>([\\s\\S]*?)<\\/[A-Za-z][\\w]*:"+name+">");
    const m=xml.match(regex);
    result[name]={present:!!m,sha256:m?SHA(Buffer.from(m[1],"utf8")):null};
  }
  return result;
}
function difference(a,b){
  const changed=[];
  for(const k of Object.keys(a))if(JSON.stringify(a[k])!==JSON.stringify(b[k]))changed.push(k);
  return changed;
}
function copyRun(dir,index){
  const target=path.resolve(dir);
  if(!path.basename(target).startsWith("run-") ||
    path.basename(path.dirname(target))!=="m4c7-psd-photoshop-save")
    throw Error("only audited M4c-7d scratch runs supported");
  const record=HISTORY.runs[index];
  const filenames=["nested-group-control-saved.psd",
    "nested-group-candidate-saved.psd"];
  const expected=[record.files_sha256.control_saved,
    record.files_sha256.candidate_saved];
  const bufs=filenames.map((file,i)=>{
    const full=path.join(target,file),info=fs.lstatSync(full);
    if(!info.isFile()||info.isSymbolicLink()||info.size>8*1024*1024)
      throw Error("unapproved scratch input");
    const bytes=fs.readFileSync(full);
    if(SHA(bytes)!==expected[i])throw Error("previously frozen M4c-7d file SHA mismatch");
    return bytes;
  });
  const a=scan(bufs[0]),b=scan(bufs[1]);
  const ta=layerTimeFields(bufs[0],a),tb=layerTimeFields(bufs[1],b);
  if(JSON.stringify(ta.map(x=>x.raw_layer_index))!==JSON.stringify(tb.map(x=>x.raw_layer_index)))
    throw Error("metadata identity differs");
  const times=ta.map((v,i)=>({
    raw_layer_index:v.raw_layer_index,
    control_value:v.timestamp_like_float64,
    candidate_value:tb[i].timestamp_like_float64,
    delta_seconds:tb[i].timestamp_like_float64-v.timestamp_like_float64,
    prefix_sha_equal:v.fixed_prefix_sha256===tb[i].fixed_prefix_sha256,
    metadata_opaque_semantics_certified:false
  }));
  const xmpA=extractXmp(resourceData(bufs[0],a,1060)),
    xmpB=extractXmp(resourceData(bufs[1],b,1060));
  const exifA=resourceData(bufs[0],a,1058),exifB=resourceData(bufs[1],b,1058);
  const exifBytePositions=[];
  for(let i=0;i<Math.min(exifA.length,exifB.length);i++)
    if(exifA[i]!==exifB[i])exifBytePositions.push(i);
  return {
    run_index:index+1,
    source_file_sha256:expected,
    layer_time:times,
    xmp:{marker_fields_differing:difference(xmpA,xmpB),
      all_XMP_properties_semantically_verified:false},
    exif:{control_length:exifA.length,candidate_length:exifB.length,
      number_of_differing_bytes:exifBytePositions.length,
      differing_byte_offsets:exifBytePositions.slice(0,12),
      tiff_signature:exifA.subarray(0,4).toString("hex"),
      exif_content_semantics_certified:false}
  };
}
function build(dirs){
  if(!Array.isArray(dirs)||dirs.length!==2||dirs[0]===dirs[1])
    throw Error("two distinct M4c-7d research run directories needed");
  const runs=dirs.map((d,i)=>copyRun(d,i));
  assert.ok(runs.every(r=>r.layer_time.length===4));
  assert.ok(runs.every(r=>r.layer_time.every(x=>x.prefix_sha_equal)));
  return {
    schema_version:"1",milestone:"M4c-9",
    status:"partially_decoded_opaque_metadata_not_certified",
    public_write_authorized:false,
    summary:{paired_runs:runs.length,custom_layer_time_items:runs.reduce((n,r)=>n+r.layer_time.length*2,0),
      all_custom_metadata_structural_prefix_equal:runs.every(r=>r.layer_time.every(x=>x.prefix_sha_equal)),
      full_EXIF_XMP_shmd_semantic_fidelity_certified:false,production_writes_authorized:0},
    limitations:[
      "cust/layerTime descriptor shape identified for this fixture, timestamp semantics not globally certified",
      "XMP DocumentID/InstanceID/History must not be blindly normalized away",
      "EXIF TIFF payload only byte-level inspected; no blanket preservation waiver",
      "Photoshop save may legitimately update document history but arbitrary unknown metadata cannot be ignored"
    ],
    runs
  };
}
function check(r){
  assert.equal(r.milestone,"M4c-9");
  assert.equal(r.public_write_authorized,false);
  assert.equal(r.summary.paired_runs,2);
  assert.equal(r.summary.custom_layer_time_items,16);
  assert.equal(r.summary.full_EXIF_XMP_shmd_semantic_fidelity_certified,false);
  assert.equal(r.summary.production_writes_authorized,0);
}
if(require.main===module){
  const mode=process.argv[2];
  if(mode==="--write"&&process.argv.length===5){
    const data=build(process.argv.slice(3));check(data);
    fs.writeFileSync(OUTPUT,JSON.stringify(data,null,2)+"\n");
    console.log(JSON.stringify({mode,...data.summary}));
  }else if(mode==="--check"&&process.argv.length===3){
    const data=JSON.parse(fs.readFileSync(OUTPUT,"utf8"));check(data);
    console.log(JSON.stringify({mode,offline:true,...data.summary}));
  }else{
    process.stderr.write("usage: node m4c9_metadata_semantics.cjs --write RUN1 RUN2 | --check\n");
    process.exitCode=2;
  }
}
module.exports={layerTimeFields,extractXmp,copyRun,build,check};
