"use strict";

// Strict research-only comparison of the two outputs after Photoshop
// independently saves an original control and a name-patched candidate.
// This is not an editor fidelity certificate, nor an output publisher.
const { scan } = require("./m4c3_raw_block_inventory.cjs");
const { decodeLuni } = require("./m4c5_byte_patch_spike.cjs");

function diff(left, right, key, fingerprint) {
  const indexed=items=>{
    const occurrence=new Map(),out=new Map();
    for(const item of items){
      const k=key(item),n=occurrence.get(k)||0;
      occurrence.set(k,n+1);out.set(k+"#"+n,fingerprint(item));
    }
    return out;
  };
  const a=indexed(left),b=indexed(right),added=[],removed=[],changed=[];
  for(const [id,value] of a){
    if(!b.has(id))removed.push(id);
    else if(b.get(id)!==value)changed.push(id);
  }
  for(const id of b.keys())if(!a.has(id))added.push(id);
  return {added,removed,changed};
}
function unchanged(result) {
  return result.added.length===0 && result.removed.length===0 &&
    result.changed.length===0;
}
function fingerprintChannels(inventory) {
  return inventory.layer_records.flatMap(r=>r.channels.map((ch,j)=>({
    key:r.index+":"+j+":"+ch.id, sha256:ch.data_sha256
  })));
}
function expectedNameRanges(bytes, inventory, rawIndex) {
  const tag=inventory.tagged_blocks.find(t=>t.scope==="layer"&&
    t.layer_index===rawIndex&&t.key==="luni");
  const blend=inventory.sections.find(s=>s.name===`layer_${rawIndex}_blending_ranges`);
  if(!tag || !blend)throw Error("missing target raw name section");
  const unicodeStart=tag.offset+8+tag.length_width+4;
  const units=bytes.readUInt32BE(tag.offset+8+tag.length_width);
  if(units<1||units>80)throw Error("unsafe Unicode name unit count");
  const unicodeEnd=unicodeStart+units*2;
  const pascalCountOffset=blend.offset+blend.length;
  const legacyLength=bytes.readUInt8(pascalCountOffset);
  const legacyStart=pascalCountOffset+1;
  const legacyEnd=legacyStart+legacyLength;
  if(unicodeEnd>bytes.length||legacyEnd>bytes.length)throw Error("invalid raw name range");
  return [{start:unicodeStart,end:unicodeEnd},{start:legacyStart,end:legacyEnd}];
}
function byteDifferenceProof(a,b,ranges) {
  if(a.length!==b.length)return {
    file_size_equal:false,changed_name_byte_count:0,changed_non_name_byte_count:Math.abs(a.length-b.length),
  };
  let changedName=0,changedElsewhere=0;
  for(let offset=0;offset<a.length;offset++){
    if(a[offset]===b[offset])continue;
    if(ranges.some(x=>offset>=x.start&&offset<x.end))changedName++;
    else changedElsewhere++;
  }
  return {file_size_equal:true,changed_name_byte_count:changedName,
    changed_non_name_byte_count:changedElsewhere};
}
function compareSavedPair(controlBytes,candidateBytes,proposal) {
  const before=scan(controlBytes),after=scan(candidateBytes);
  const record=proposal.raw_index;
  const beforeTarget=before.layer_records[record];
  const afterTarget=after.layer_records[record];
  if(!beforeTarget || !afterTarget || !Number.isInteger(record) || record<0){
    throw Error("missing target raw record");
  }
  const oldTags=before.tagged_blocks.filter(t=>t.scope==="layer" &&
    t.layer_index===record&&t.key==="luni");
  const newTags=after.tagged_blocks.filter(t=>t.scope==="layer" &&
    t.layer_index===record&&t.key==="luni");
  if(oldTags.length!==1 || newTags.length!==1)throw Error("target luni is not unique");
  const oldName=decodeLuni(controlBytes,oldTags[0]).value;
  const newName=decodeLuni(candidateBytes,newTags[0]).value;
  const ranges=expectedNameRanges(controlBytes,before,record);
  const byteProof=byteDifferenceProof(controlBytes,candidateBytes,ranges);
  const header_equal=JSON.stringify(before.header)===JSON.stringify(after.header);
  const resources=diff(before.image_resources,after.image_resources,
    item=>item.id+":"+item.name,item=>item.data_sha256);
  const channels=diff(fingerprintChannels(before),fingerprintChannels(after),
    item=>item.key,item=>item.sha256);
  const tags=diff(before.tagged_blocks,after.tagged_blocks,
    item=>item.scope+":"+String(item.layer_index??"-")+":"+item.key,
    item=>item.data_sha256);
  const legacyChanged=[];
  const structureChanged=[];
  for(let i=0;i<Math.max(before.layer_records.length,after.layer_records.length);i++){
    const x=before.layer_records[i],y=after.layer_records[i];
    if(!x||!y){structureChanged.push(i);continue;}
    const fields=r=>JSON.stringify({bounds:r.bounds,channels:r.channel_count,
      blend:r.blend_mode,opacity:r.opacity,clipping:r.clipping,flags:r.flags});
    if(fields(x)!==fields(y))structureChanged.push(i);
    if(x.legacy_name!==y.legacy_name)legacyChanged.push(i);
  }
  const taggedTargetKey="layer:"+record+":luni#0";
  const evidence={
    status:"research_pair_compared",
    public_write_authorized:false,
    old_unicode_name:oldName,
    candidate_unicode_name:newName,
    old_pascal_name:beforeTarget.legacy_name,
    candidate_pascal_name:afterTarget.legacy_name,
    header_equal, resources,channels,tags,
    structure_changed_raw_indices:structureChanged,
    legacy_name_changed_raw_indices:legacyChanged,
    merged_composite_equal:before.merged_image.data_sha256===
      after.merged_image.data_sha256,
    source_format_equal:before.header.format===after.header.format,
    byte_difference_proof:byteProof,
  };
  evidence.strict_saved_pair_bytes_passed=
    oldName===proposal.expected_old_name &&
    newName===proposal.expected_new_name &&
    beforeTarget.legacy_name===proposal.expected_old_name &&
    afterTarget.legacy_name===proposal.expected_new_name &&
    header_equal && evidence.source_format_equal &&
    before.layer_records.length===after.layer_records.length &&
    unchanged(resources)&&unchanged(channels) &&
    tags.added.length===0&&tags.removed.length===0 &&
    tags.changed.length===1&&tags.changed[0]===taggedTargetKey &&
    structureChanged.length===0 &&
    legacyChanged.length===1&&legacyChanged[0]===record &&
    evidence.merged_composite_equal &&
    byteProof.file_size_equal&&byteProof.changed_name_byte_count>0 &&
    byteProof.changed_non_name_byte_count===0;
  return evidence;
}
module.exports={compareSavedPair,diff};
