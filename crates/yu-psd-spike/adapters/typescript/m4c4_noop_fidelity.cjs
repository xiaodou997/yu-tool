"use strict";

// M4c-4 research-only no-op writer comparison. The output files are created
// only in ignored target/ and never published to a user's PSD destination.
// Exact raw-byte changes are evidence, not by themselves semantic data loss.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const os=require("node:os");
const {scan}=require("./m4c3_raw_block_inventory.cjs");
const {scanNested}=require("./m4c4_nested_layer_inventory.cjs");

const ROOT=path.resolve(__dirname,"../../../../");
const DEST=path.join(ROOT,"target/m4c4-psd-fidelity");
const REPORT=path.join(ROOT,"docs/data/m4c4-psd-noop-fidelity-v1.json");
const PACKAGE=path.join(ROOT,"packaging/ag-psd-engine/node_modules/ag-psd");
const psdFiles=[
  ["fixtures/psd","fixtures/psd/corpus.json"],
  ["fixtures/psd/benchmark","fixtures/psd/benchmark/corpus.json"],
];
const sha=x=>crypto.createHash("sha256").update(x).digest("hex");
const errorText=e=>String(e?.message||e).slice(0,500);

function indexed(records,keyFn){
  const counts=new Map(),entries=new Map();
  for(const record of records){
    const name=keyFn(record),i=counts.get(name)||0;
    counts.set(name,i+1);
    entries.set(`${name}#${i}`,record);
  }
  return entries;
}
function delta(before,after,keyFn,fieldFn,limit=20){
  const left=indexed(before,keyFn),right=indexed(after,keyFn);
  const added=[],removed=[],changed=[];
  for(const [key,obj] of left) {
    if(!right.has(key))removed.push(key);
    else if(fieldFn(obj)!==fieldFn(right.get(key)))changed.push(key);
  }
  for(const key of right.keys())if(!left.has(key))added.push(key);
  return {added_count:added.length,removed_count:removed.length,
    changed_count:changed.length,
    added:added.slice(0,limit),removed:removed.slice(0,limit),
    changed:changed.slice(0,limit)};
}
const changed=d=>!!(d.added_count||d.removed_count||d.changed_count);
const hashOrNull=x=>x?sha(Buffer.from(x.data.buffer,x.data.byteOffset,x.data.byteLength)):null;

function semantic(psd){
  const layers=[];
  function walk(children){
    for(const x of children||[]){
      layers.push({
        name:x.name??"",kind:Array.isArray(x.children)?"group":x.text?"text":
          x.placedLayer?"smart_object":"pixel",
        hidden:x.hidden===true,opacity:x.opacity??1,
        blend:x.blendMode??null,
        geometry:[x.left??null,x.top??null,x.right??null,x.bottom??null],
        image_sha256:hashOrNull(x.imageData),
        mask_sha256:hashOrNull(x.mask?.imageData),
      });
      walk(x.children);
    }
  }
  walk(psd.children);
  return {composite_sha256:hashOrNull(psd.imageData),layers};
}

function compare(before,after) {
  const resources=delta(before.image_resources,after.image_resources,
    r=>String(r.id)+":"+r.name,r=>r.data_sha256);
  const rawTags=delta(before.tagged_blocks,after.tagged_blocks,
    t=>`${t.scope}:${t.layer_index??"-"}:${t.key}`,t=>t.data_sha256);
  const channelItems=x=>x.layer_records.flatMap(r=>
    r.channels.map((ch,j)=>({id:`${r.index}:${j}:${ch.id}`,sha256:ch.data_sha256})));
  const channels=delta(channelItems(before),channelItems(after),
    a=>a.id,a=>a.sha256);
  const props=delta(before.layer_records,after.layer_records,
    r=>String(r.index),r=>JSON.stringify({
      bounds:r.bounds,blend:r.blend_mode,opacity:r.opacity,clipping:r.clipping,
      flags:r.flags,legacy_name:r.legacy_name,ids:r.channels.map(ch=>ch.id)
    }));
  const relevantSections=x=>x.sections.filter(s=>
    ["color_mode_data","image_resources","layer_info","global_layer_mask_data"].includes(s.name));
  const sections=delta(relevantSections(before),relevantSections(after),
    s=>s.name,s=>s.data_sha256);
  const unknown=delta(before.unknown_blocks,after.unknown_blocks,
    b=>`${b.category}:${b.scope??"-"}:${b.key??b.id}`,
    b=>JSON.stringify(b).replace(/"offset":\d+,?/g,""));
  // Nested records may exist in PSB high-bit Additional Layer Information.
  const aNest=scanNested(Buffer.from(before.__bytes),before);
  const bNest=scanNested(Buffer.from(after.__bytes),after);
  const nestedLayers=x=>x.nested.flatMap(n=>n.records.map(r=>({
    id:`${n.scope}:${n.key}:${r.index}`,
    props:JSON.stringify([r.bounds,r.blend_mode,r.opacity,r.flags,r.legacy_name]),
    channelHashes:r.channels.map(ch=>ch.data_sha256),
  })));
  const nestedProps=delta(nestedLayers(aNest),nestedLayers(bNest),
    l=>l.id,l=>l.props);
  const nestedChannels=delta(nestedLayers(aNest),nestedLayers(bNest),
    l=>l.id,l=>JSON.stringify(l.channelHashes));
  return {
    raw_source_sha256:before.source.sha256,
    raw_output_sha256:after.source.sha256,
    byte_equal:before.source.sha256===after.source.sha256,
    source_bytes:before.source.bytes,output_bytes:after.source.bytes,
    same_header:JSON.stringify(before.header)===JSON.stringify(after.header),
    resources,additional_layer_blocks:rawTags,channels,layer_properties:props,
    sections,unknown,nested_layer_properties:nestedProps,nested_channel_bytes:nestedChannels,
    merged_composite_byte_equal:
      before.merged_image.data_sha256===after.merged_image.data_sha256,
    nested_before:aNest.nested_layer_records,nested_after:bNest.nested_layer_records,
    differences_found:[resources,rawTags,channels,props,sections,unknown,
      nestedProps,nestedChannels].some(changed) ||
      before.merged_image.data_sha256!==after.merged_image.data_sha256 ||
      before.source.sha256!==after.source.sha256,
  };
}

function bytesInventory(bytes){
  const inventory=scan(bytes);
  // Private in-memory carrier, stripped from the evidence before serialization.
  Object.defineProperty(inventory,"__bytes",{value:bytes,enumerable:false});
  return inventory;
}
function main(){
  if(process.versions.node!=="22.23.3")throw Error("pinned Node 22.23.3 required");
  const ag=require(PACKAGE);
  if(require(path.join(PACKAGE,"package.json")).version!=="31.0.2"){
    throw Error("pinned ag-psd 31.0.2 required");
  }
  ag.initializeCanvas(
    ()=>{throw Error("browser canvas is prohibited");},
    (w,h)=>{if(w*h>2_000_000||w<=0||h<=0)throw Error("pixel budget exceeded");
      return {width:w,height:h,data:new Uint8ClampedArray(w*h*4)};}
  );
  const opts={useImageData:true,useRawThumbnail:true,
    // Unlike M4c-1, do NOT intentionally skip linked-file payloads.
    skipLinkedFilesData:false,logMissingFeatures:false};
  fs.mkdirSync(DEST,{recursive:true});
  // Unique ignored research directory; no existing staged artifact can be
  // replaced by repeated tests. Report content intentionally omits run paths.
  const runDir=fs.mkdtempSync(path.join(DEST,"run-"));
  const cases=[];
  for(const [root,manifestFile] of psdFiles){
    const fixtures=JSON.parse(fs.readFileSync(path.join(ROOT,manifestFile),"utf8")).fixtures;
    for(const fixture of fixtures){
      if(fixture.expected?.parse==="reject")continue;
      const input=path.join(ROOT,root,fixture.path);
      const source=fs.readFileSync(input),originalSha=sha(source);
      const item={fixture:fixture.id,input:path.relative(ROOT,input).split(path.sep).join("/"),
        source_sha256:originalSha,status:"not_tested",public_write_authorized:false};
      try{
        const a=bytesInventory(source);
        const doc=ag.readPsd(source,opts);
        const semanticBefore=semantic(doc);
        const output=ag.writePsdBuffer(doc,{psb:a.header.version===2});
        if(!Buffer.isBuffer(output)||output.length>8*1024*1024) {
          throw Error("writer did not produce bounded Buffer");
        }
        const filename=`${fixture.id}.${a.header.format}`;
        const destination=path.join(runDir,filename);
        // Neither source path nor repository fixture is passed to the writer.
        fs.writeFileSync(destination,output,{flag:"wx"});
        const disk=fs.readFileSync(destination);
        if(sha(disk)!==sha(output))throw Error("disk output differs from written output");
        const b=bytesInventory(disk);
        const saved=ag.readPsd(disk,opts);
        const semanticAfter=semantic(saved);
        const diff=compare(a,b);
        item.status="saved_reopened";
        item.header_format=a.header.format;
        item.bits_per_channel=a.header.bit_depth;
        item.raw_diff=diff;
        item.semantic_check={
          composite_pixels_equal:
            semanticBefore.composite_sha256===semanticAfter.composite_sha256,
          logical_layer_count_equal:
            semanticBefore.layers.length===semanticAfter.layers.length,
          layer_attributes_equal:
            semanticBefore.layers.every((layer,i)=>{
              const after=semanticAfter.layers[i];
              return !!after && JSON.stringify({...layer,image_sha256:null,mask_sha256:null})===
                JSON.stringify({...after,image_sha256:null,mask_sha256:null});
            }),
          decoded_layer_image_hashes_equal:
            semanticBefore.layers.length===semanticAfter.layers.length &&
            semanticBefore.layers.every((layer,i)=>
              layer.image_sha256===semanticAfter.layers[i]?.image_sha256),
        };
        item.semantic_check.any_mismatch=Object.entries(item.semantic_check)
          .some(([k,v])=>k!=="any_mismatch"&&!v);
        // A raw mismatch does not prove visual loss, but blocks preservation
        // certification until a complete independent audit has classified it.
        item.unexpected_raw_changes=diff.differences_found;
        item.fidelity_certified=false;
      }catch(e){
        item.status="failed_closed";
        item.error=errorText(e);
      }
      if(sha(fs.readFileSync(input))!==originalSha)throw Error("source input unexpectedly mutated");
      item.source_unchanged=true;
      cases.push(item);
    }
  }
  cases.sort((a,b)=>a.fixture.localeCompare(b.fixture,"en"));
  const report={
    schema_version:"1",milestone:"M4c-4",public_write_authorized:false,
    runtime:{node:process.versions.node,ag_psd:"31.0.2",platform:os.platform(),arch:os.arch()},
    summary:{
      fixture_count:cases.length,
      saved_reopened:cases.filter(x=>x.status==="saved_reopened").length,
      failed_closed:cases.filter(x=>x.status==="failed_closed").length,
      raw_diff_found:cases.filter(x=>x.status==="saved_reopened"&&x.raw_diff.differences_found).length,
      composite_encoded_bytes_changed:cases.filter(x=>x.status==="saved_reopened"&&!x.raw_diff.merged_composite_byte_equal).length,
      semantic_composite_mismatch:cases.filter(x=>x.status==="saved_reopened"&&!x.semantic_check.composite_pixels_equal).length,
      fidelity_certified:0,
    },
    cases,
  };
  assert.equal(cases.length,13);
  assert.ok(cases.every(c=>c.source_unchanged&&!c.public_write_authorized));
  return JSON.stringify(report,null,2)+"\n";
}

if(require.main===module){
  const mode=process.argv[2];
  if(!["--write","--check"].includes(mode)||process.argv.length!==3){
    process.stderr.write("usage: node m4c4_noop_fidelity.cjs --write|--check\n");
    process.exit(2);
  }
  const report=main();
  if(mode==="--write")fs.writeFileSync(REPORT,report);
  else assert.equal(fs.readFileSync(REPORT,"utf8"),report,"M4c-4 evidence changed");
  console.log(JSON.stringify({mode,...JSON.parse(report).summary}));
}
module.exports={main,compare,bytesInventory};
