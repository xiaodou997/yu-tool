"use strict";

// Optional M4c-7d *native-render* reference. Photoshop opens only exact,
// isolated M4c-7 saved/control PSD files and exports new PNG copies under
// ignored target/. No user files, no PSD modifications, no product writer.
const fs=require("node:fs");
const path=require("node:path");
const assert=require("node:assert/strict");
const {spawnSync}=require("node:child_process");
const {classifyPhotoshopReadiness}=require("./m4c7c_native_readiness.cjs");

const ROOT=path.resolve(__dirname,"../../../../");
const BASE=path.join(ROOT,"target/m4c7-psd-photoshop-save");
const REFS=[
  "nested-group-control.psd",
  "nested-group.psd",
  "nested-group-control-saved.psd",
  "nested-group-candidate-saved.psd",
];

function exportComposite(input,output) {
  const abs=path.resolve(input),dest=path.resolve(output);
  if(!abs.startsWith(BASE+path.sep) || !dest.startsWith(BASE+path.sep) ||
     path.dirname(abs)!==path.dirname(dest))throw Error("outside isolated run");
  const stats=fs.lstatSync(abs);
  if(!stats.isFile()||stats.isSymbolicLink()||stats.size>8*1024*1024)
    throw Error("unsafe research input");
  if(fs.existsSync(dest))throw Error("output already exists");
  const state=classifyPhotoshopReadiness();
  if(!state.ready)throw Error("Photoshop is not in an empty, responsive state: "+state.status);
  const script=[
    "(function(){var doc=null;var prev=app.displayDialogs;",
    "try{app.displayDialogs=DialogModes.NO;",
    "doc=app.open(new File("+JSON.stringify(abs)+"));",
    "var opt=new PNGSaveOptions();opt.interlaced=false;",
    "doc.saveAs(new File("+JSON.stringify(dest)+"),opt,true,Extension.LOWERCASE);",
    "return 'rendered';",
    "}finally{if(doc)doc.close(SaveOptions.DONOTSAVECHANGES);",
    "app.displayDialogs=prev;}})()"
  ].join("");
  const osa='tell application id "com.adobe.Photoshop" to do javascript '+
    JSON.stringify(script);
  const result=spawnSync("/usr/bin/osascript",["-e",osa],{
    cwd:ROOT,encoding:"utf8",timeout:90000,maxBuffer:1024*1024,
  });
  if(result.error||result.status!==0||result.stdout.trim()!=="rendered")
    throw Error("Photoshop native compositing unavailable: "+
      (result.error?.message||result.stderr||result.stdout).slice(0,300));
  const saved=fs.lstatSync(dest);
  if(!saved.isFile()||saved.size>8*1024*1024)
    throw Error("invalid native-render PNG");
  return saved.size;
}

if(require.main===module) {
  const dir=path.resolve(process.argv[2]||"");
  if(process.argv.length!==3 || path.dirname(dir)!==BASE||
     !path.basename(dir).startsWith("run-")){
    process.stderr.write("usage: node m4c7d_native_composite_probe.cjs ISOLATED_RUN_DIR\n");
    process.exitCode=2;
  } else {
    const result={output_kind:"native Photoshop PNG render of isolated PSDs",files:[]};
    for(const file of REFS){
      const png=file.replace(/\.psd$/,".native.png");
      const size=exportComposite(path.join(dir,file),path.join(dir,png));
      result.files.push({source:file,output:png,bytes:size});
    }
    assert.equal(result.files.length,4);
    console.log(JSON.stringify(result));
  }
}
module.exports={exportComposite};
