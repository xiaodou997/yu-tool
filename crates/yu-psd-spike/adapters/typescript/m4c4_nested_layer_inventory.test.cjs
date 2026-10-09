"use strict";

const {test}=require("node:test");
const assert=require("node:assert/strict");
const fs=require("node:fs");
const path=require("node:path");
const {scan,ScanError}=require("./m4c3_raw_block_inventory.cjs");
const {scanNested}=require("./m4c4_nested_layer_inventory.cjs");

const ROOT=path.resolve(__dirname,"../../../../");
const PSB="fixtures/psd/benchmark/upstream/psd-tools/32bit.psb";
const PSD16="fixtures/psd/benchmark/upstream/psd-tools/posterize-16bit-rgb.psd";
const bytes=f=>fs.readFileSync(path.join(ROOT,f));

test("PSB Lr32 has two real nested layers with measured byte offsets",()=>{
  const source=bytes(PSB);
  const inv=scan(source),result=scanNested(source,inv);
  assert.equal(result.mutation_authorized,false);
  assert.equal(result.safe_to_rewrite,false);
  assert.equal(result.identified_nested_blocks,1);
  assert.equal(result.nested_layer_records,2);
  assert.equal(result.nested[0].key,"Lr32");
  assert.equal(result.nested[0].records.length,2);
  for(const layer of result.nested[0].records){
    assert.ok(layer.channels.length>0);
    for(const channel of layer.channels){
      assert.match(channel.data_sha256,/^[0-9a-f]{64}$/);
      assert.equal(channel.data_sha256,
        require("node:crypto").createHash("sha256").update(
          source.subarray(channel.offset,channel.offset+channel.length)
        ).digest("hex"));
    }
    assert.ok(layer.tags.some(t=>t.key==="luni"));
  }
});

test("16-bit PSD Lr16 exposes eleven nested records but cannot authorize saving",()=>{
  const result=scanNested(bytes(PSD16));
  assert.equal(result.nested_layer_records,11);
  assert.equal(result.nested[0].key,"Lr16");
  assert.equal(result.nested[0].record_count,11);
  assert.equal(result.safe_to_rewrite,false);
  assert.ok(result.risks.includes("opaque_channel_payloads_not_decoded_or_verified"));
});

test("normal 8-bit PSD without embedded Layr/Lr16/Lr32 remains unchanged",()=>{
  const b=bytes("fixtures/psd/upstream/psd-tools/2layers.psd");
  const baseline=scan(b);
  const nested=scanNested(b,baseline);
  assert.equal(nested.identified_nested_blocks,0);
  assert.equal(nested.nested_layer_records,0);
  assert.equal(nested.source.sha256,baseline.source.sha256);
});

test("embedded layer-info bounds reject malicious layer count and channel length",()=>{
  const original=bytes(PSB),baseline=scan(original);
  const key=baseline.tagged_blocks.find(x=>x.key==="Lr32");
  assert.ok(key);
  const start=key.offset+8+key.length_width;
  let evil=Buffer.from(original);
  evil.writeInt16BE(65,start);
  // A changed input must not be paired with an old inventory.
  assert.throws(()=>scanNested(evil,baseline),e=>
    e instanceof ScanError&&e.code==="INVALID_ARGUMENT");
  assert.throws(()=>scanNested(evil),e=>
    e instanceof ScanError&&e.code==="UNSUPPORTED_DOCUMENT");

  evil=Buffer.from(original);
  // First channel length starts after layer count, bounds and channel count,
  // followed by its signed 16-bit channel ID.
  const firstLength=start+2+16+2+2;
  evil.writeBigUInt64BE(BigInt(Number.MAX_SAFE_INTEGER)+1n,firstLength);
  assert.throws(()=>scanNested(evil),e=>
    e instanceof ScanError&&e.code==="UNSUPPORTED_DOCUMENT");

  evil=Buffer.from(original);
  evil.write("BAD!",start+2+16+2+3*10,4,"ascii");
  assert.throws(()=>scanNested(evil),e=>e instanceof ScanError);
});

test("multiple corpus fixtures can be examined read-only without claiming fidelity",()=>{
  for(const f of [PSB,PSD16,
    "fixtures/psd/upstream/psd-tools/group.psd",
    "fixtures/psd/benchmark/upstream/psd-tools/advanced-blending.psd"]){
    const source=bytes(f);
    const digest=require("node:crypto").createHash("sha256").update(source).digest("hex");
    const output=scanNested(source);
    assert.equal(output.source.sha256,digest);
    assert.equal(output.mutation_authorized,false);
    assert.equal(output.safe_to_rewrite,false);
    assert.ok(output.nested_layer_records>=0);
  }
});
