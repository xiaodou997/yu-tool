"use strict";

// M4c-4: bounded, read-only inventory of embedded Layr / Lr16 / Lr32
// additional-info payloads. This extends M4c-3's structural scanner without
// altering its frozen machine-readable evidence or public protocol.
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const { scan, ScanError } = require("./m4c3_raw_block_inventory.cjs");

const MAX_NEST = 4;
const MAX_LAYERS = 64;
const MAX_CHANNELS = 128;
const MAX_TAGS = 1024;
const LONG_KEYS = new Set([
  "LMsk", "Lr16", "Lr32", "Layr", "Mt16", "Mt32", "Mtrn",
  "Alph", "FMsk", "lnk2", "FEid", "FXid", "PxSD"
]);
const EMBEDDED = new Set(["Layr", "Lr16", "Lr32"]);
const sha = x => crypto.createHash("sha256").update(x).digest("hex");

function bad(message, offset, code = "INVALID_INPUT") {
  throw new ScanError(code, message, offset);
}

function reader(bytes, start, end, label) {
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) ||
      start < 0 || start > end || end > bytes.length) bad(`invalid ${label} bounds`, start);
  const c = { pos: start, end, bytes, label };
  c.need = (n, what) => {
    if (!Number.isSafeInteger(n) || n < 0 || n > c.end - c.pos) {
      bad(`truncated ${label}: ${what}`, c.pos);
    }
  };
  c.uint8 = () => { c.need(1,"u8"); return bytes[c.pos++]; };
  c.uint16 = () => { c.need(2,"u16"); const n=bytes.readUInt16BE(c.pos);c.pos+=2;return n; };
  c.int16 = () => { c.need(2,"i16"); const n=bytes.readInt16BE(c.pos);c.pos+=2;return n; };
  c.int32 = () => { c.need(4,"i32"); const n=bytes.readInt32BE(c.pos);c.pos+=4;return n; };
  c.uint32 = () => { c.need(4,"u32"); const n=bytes.readUInt32BE(c.pos);c.pos+=4;return n; };
  c.uint64 = () => {
    c.need(8,"u64");
    const n=bytes.readBigUInt64BE(c.pos);
    c.pos+=8;
    if (n>BigInt(Number.MAX_SAFE_INTEGER)) bad("u64 length exceeds safe integer",c.pos-8,"UNSUPPORTED_DOCUMENT");
    return Number(n);
  };
  c.ascii = n => { c.need(n,"ASCII");const v=bytes.toString("latin1",c.pos,c.pos+n);c.pos+=n;return v; };
  c.take = n => { c.need(n,"payload");const start=c.pos;c.pos+=n;return {start,end:c.pos,hash:sha(bytes.subarray(start,c.pos))}; };
  c.sub = (n,subLabel) => {c.need(n,subLabel);const out=reader(bytes,c.pos,c.pos+n,subLabel);c.pos+=n;return out; };
  c.section32 = label => c.sub(c.uint32(),label);
  c.pascal = align => {
    const start=c.pos, len=c.uint8();
    const str=c.ascii(len);
    const pad=(align-((c.pos-start)%align))%align;
    c.take(pad);return str;
  };
  c.left = () => c.end-c.pos;
  return c;
}

function scanNested(bytes, inventory = scan(bytes)) {
  if (!Buffer.isBuffer(bytes)) bad("input must be Buffer",0,"INVALID_ARGUMENT");
  if (sha(bytes)!==inventory.source.sha256 || inventory.source.bytes!==bytes.length) {
    bad("inventory does not match input bytes",0,"INVALID_ARGUMENT");
  }
  const version=inventory.header.version;
  const nested = [], anomalies=[];
  let globalTags=0, globalLayers=0;
  function walk(start,end,key,scope,depth) {
    if(depth>MAX_NEST) bad("nested layer-info depth exceeds limit",start,"UNSUPPORTED_DOCUMENT");
    const c=reader(bytes,start,end,`embedded ${key}`);
    if (!c.left()) bad("embedded layer-info is empty",start);
    const layerCount = c.int16();
    const n=Math.abs(layerCount);
    if(n>MAX_LAYERS || globalLayers+n>MAX_LAYERS) {
      bad("nested layer record count exceeds limit",c.pos-2,"UNSUPPORTED_DOCUMENT");
    }
    let records=[],channelSequence=[];
    for(let i=0;i<n;i++){
      const offset=c.pos;
      const bounds=[c.int32(),c.int32(),c.int32(),c.int32()];
      const channels=c.uint16();
      if(channels>MAX_CHANNELS) bad("nested layer channel count exceeds limit",c.pos-2,"UNSUPPORTED_DOCUMENT");
      const ch=[];
      for(let j=0;j<channels;j++) {
        const id=c.int16();
        const len=version===2?c.uint64():c.uint32();
        const current={id,length:len};
        ch.push(current);channelSequence.push(current);
      }
      const blendSignature=c.ascii(4);
      if(blendSignature!=="8BIM")bad("invalid nested layer blend signature",c.pos-4);
      const blendMode=c.ascii(4);
      const opacity=c.uint8(),clipping=c.uint8(),flags=c.uint8();
      c.take(1);
      const extra=c.section32(`nested ${key} layer extra`);
      const mask=extra.section32("nested mask");mask.take(mask.left());
      const blending=extra.section32("nested blending");blending.take(blending.left());
      const legacyName=extra.pascal(4);
      const tags=[];
      while(extra.left()>0){
        const tagOffset=extra.pos;
        if (extra.left()>=6 && bytes.readUInt16BE(extra.pos)===0 &&
            ["8BIM","8B64"].includes(bytes.toString("latin1",extra.pos+2,extra.pos+6))) {
          extra.take(2);anomalies.push({scope,offset:tagOffset,type:"tag_prefix_padding"});
        }
        if(++globalTags>MAX_TAGS)bad("nested tagged block count exceeds limit",extra.pos,"UNSUPPORTED_DOCUMENT");
        const sig=extra.ascii(4);
        if(!["8BIM","8B64"].includes(sig))bad("invalid nested tagged signature",tagOffset);
        const tkey=extra.ascii(4);
        const lengthWidth=version===2&&LONG_KEYS.has(tkey)?8:4;
        const len=lengthWidth===8?extra.uint64():extra.uint32();
        const payload=extra.take(len);
        extra.take(len%2);
        const tag={signature:sig,key:tkey,offset:tagOffset,length_width:lengthWidth,
          length:len,data_sha256:payload.hash};
        tags.push(tag);
        if(EMBEDDED.has(tkey)) {
          const nestedBlock=walk(payload.start,payload.end,tkey,`${scope}/${i}/${tkey}`,depth+1);
          tag.embedded_layer_count=nestedBlock.record_count;
        }
      }
      records.push({index:i,offset,bounds,blend_mode:blendMode,opacity,
        clipping,flags,legacy_name:legacyName,channels:ch,tags});
      globalLayers++;
    }
    for(const ch of channelSequence){
      const pos=c.pos,blob=c.take(ch.length);
      ch.offset=pos;ch.data_sha256=blob.hash;
    }
    const tail=c.left();
    if(tail && tail<=3 && bytes.subarray(c.pos,c.end).every(b=>b===0)){
      c.take(tail);anomalies.push({scope,offset:c.end-tail,type:"layer_info_zero_padding",bytes:tail});
    }
    if(c.left())bad("nested layer data contains unexpected trailing bytes",c.pos);
    const result={scope,key,depth,offset:start,length:end-start,
      signed_layer_count:layerCount,record_count:n,
      records,sha256:sha(bytes.subarray(start,end))};
    nested.push(result);
    return result;
  }
  for(const tag of inventory.tagged_blocks) {
    if(!EMBEDDED.has(tag.key))continue;
    const offset=tag.offset+8+tag.length_width;
    const end=offset+tag.length;
    if(end>bytes.length)bad("embedded tag data exceeds source",offset);
    walk(offset,end,tag.key,tag.scope,1);
  }
  nested.sort((a,b)=>a.offset-b.offset);
  return {
    schema_version:"1",operation:"psd.raw.nested.inspect.research",
    source:inventory.source,format:inventory.header.format,
    mutation_authorized:false,safe_to_rewrite:false,
    identified_nested_blocks:nested.length,
    nested_layer_records:nested.reduce((sum,item)=>sum+item.record_count,0),
    nested,anomalies,
    risks:["nested_records_parsed_not_semantically_preservation_audited",
      "opaque_channel_payloads_not_decoded_or_verified"].sort()
  };
}

function inspectNestedFile(file) {
  const p=path.resolve(file);
  const stat=fs.lstatSync(p);
  if(!stat.isFile()||stat.size>8*1024*1024)bad("invalid or oversized PSD/PSB",0,"UNSUPPORTED_DOCUMENT");
  const b=fs.readFileSync(p);
  return scanNested(b);
}

module.exports={scanNested,inspectNestedFile,ScanError};
