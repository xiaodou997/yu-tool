"use strict";

// M4c-13: strict READ-ONLY reconciliation and manual-disposition plan.
// No auto link/retry/cleanup and no "receipt repair" for an output
// whose original publisher was interrupted. All routes are research-only.
const fs=require("node:fs");
const path=require("node:path");
const {newRun,attempt,recovery}=require("./m4c12_recovery_fs_gate.cjs");
const ROOT=path.resolve(__dirname,"../../../../");
const REPORT=path.join(ROOT,"docs/data/m4c13-manual-recovery-v1.json");

const GUIDANCE={
  staged_only_manual_disposition_required:{
    decision:"HOLD_STAGED_ONLY",
    operator_steps:[
      "Confirm that the transaction belongs to the pinned research source, not a customer PSD",
      "Inspect the private intent and staged SHA-256; no output was observed",
      "Do not automatically retry, rename, delete the stage or publish new output",
      "Open a new separately authorized research transaction if a fresh output is still required"
    ]
  },
  published_candidate_without_receipt_reinspect_only:{
    decision:"HOLD_PUBLISHED_NO_VERIFIED_RECEIPT",
    operator_steps:[
      "Preserve the existing published inode and private stage unchanged",
      "Record source SHA-256 and inode, output SHA-256, stage/output device and inode identity",
      "Treat earlier publication acknowledgement and crash durability as INCONCLUSIVE",
      "Obtain a separate manual evidence and release decision; never fabricate a success receipt"
    ]
  },
  verified_receipt_observed:{
    decision:"OBSERVED_COMPLETED_RESEARCH",
    operator_steps:[
      "Confirm the existing research-only receipt and current output device/inode and SHA-256",
      "Preserve the original transaction evidence for independent review",
      "Do not infer actual power-loss recovery, source race freedom or production PSD write approval"
    ]
  },
  source_drift_refuse_automatic_resume:{
    decision:"STOP_SOURCE_VERSION_CHANGED",
    operator_steps:[
      "Stop processing and preserve all stage and output paths",
      "Investigate changed or replaced pinned research source and compare the original version",
      "Reject any attempt to reuse the old source version, output name or transaction ticket"
    ]
  },
  output_conflict_do_not_touch:{
    decision:"STOP_UNTRUSTED_OUTPUT",
    operator_steps:[
      "Do not modify, overwrite, delete or relink the conflicting target",
      "Inspect whether the output inode or content differs from the private staged candidate",
      "Treat matching bytes without matching inode as an unaffiliated competing publisher",
      "Escalate manual provenance review before authorizing any further experiment"
    ]
  },
  recovery_refused_untrusted_evidence:{
    decision:"STOP_INVALID_TRANSACTION_EVIDENCE",
    operator_steps:[
      "Do not trust the staged candidate, parsed intent or any old success receipt",
      "Preserve the original files and investigate possible corruption or replacement",
      "Do not attempt automatic cleanup, repair, deletion or publication"
    ]
  }
};
function classify(reconciled) {
  if(!reconciled||reconciled.milestone!=="M4c-12"||
    reconciled.public_write_authorized!==false ||
    reconciled.automatic_republication_attempted!==false||
    reconciled.automatic_output_deletion_attempted!==false)
    throw Error("untrusted recovery output");
  const value=GUIDANCE[reconciled.classification];
  if(!value)throw Error("unrecognized recovery classification");
  return {
    original_classification:reconciled.classification,
    ...value,
    source_version_matches:reconciled.source_sha256_still_matches,
    current_published_state:reconciled.published_status,
    private_staging_and_published_inode_match:
      reconciled.output_inode_matches_private_staging,
    publisher_receipt_previously_present:reconciled.existing_verified_receipt,
    actual_power_loss_durability_proven:false,
    no_auto_repair:true,no_auto_republish:true,no_auto_delete:true,
    production_psd_write_authorized:false
  };
}
function inspect(dir) {
  // recovery() is read-only and source/destination paths are restricted
  // by M4c-12's safeDir validation. No customer path is accepted.
  try{return classify(recovery(dir));}
  catch(e){
    if(e?.code!=="RECOVERY_AMBIGUOUS"&&
       e?.code!=="UNSAFE_FILE"&&
       e?.code!=="SOURCE_VERSION_CONFLICT"&&
       e?.code!=="RESEARCH_SCOPE_DENIED")throw e;
    return {
      original_classification:"recovery_refused_untrusted_evidence",
      ...GUIDANCE.recovery_refused_untrusted_evidence,
      original_error_code:e.code,
      actual_power_loss_durability_proven:false,
      no_auto_repair:true,no_auto_republish:true,no_auto_delete:true,
      production_psd_write_authorized:false
    };
  }
}
function build() {
  const cases=[];
  for(const cut of ["after_stage","after_link","after_directory_fsync",null]){
    const dir=newRun();
    const initial=attempt(dir,{interruptAt:cut});
    const observed=inspect(dir);
    cases.push({
      test_cutpoint:cut||"normal_completed",
      initial_research_state:initial.state,
      manual_recovery:observed
    });
  }
  return {
    schema_version:"1",milestone:"M4c-13",
    operation:"manual-only independent PSD research transaction review",
    public_write_authorized:false,
    summary:{read_only_manual_dispositions:cases.length,
      missing_receipt_explicit_manual_holds:cases.filter(x=>
        x.manual_recovery.decision==="HOLD_PUBLISHED_NO_VERIFIED_RECEIPT").length,
      automatic_recovery_writes:0,
      production_psd_writes_authorized:0},
    cases
  };
}
function check(r){
  if(r.milestone!=="M4c-13"||r.public_write_authorized!==false||
    r.summary.read_only_manual_dispositions!==4||
    r.summary.missing_receipt_explicit_manual_holds!==2 ||
    r.summary.automatic_recovery_writes!==0||
    r.summary.production_psd_writes_authorized!==0||
    !r.cases.every(x=>x.manual_recovery.no_auto_delete&&
      x.manual_recovery.no_auto_republish&&
      x.manual_recovery.production_psd_write_authorized===false))
    throw Error("manual recovery evidence must remain nonmutating");
}
if(require.main===module){
  if(process.argv[2]==="--write"&&process.argv.length===3){
    const r=build();check(r);
    fs.writeFileSync(REPORT,JSON.stringify(r,null,2)+"\n");
    console.log(JSON.stringify({mode:"--write",...r.summary}));
  }else if(process.argv[2]==="--check"&&process.argv.length===3){
    const r=JSON.parse(fs.readFileSync(REPORT,"utf8"));check(r);
    console.log(JSON.stringify({mode:"--check",offline:true,...r.summary}));
  }else if(process.argv[2]==="--inspect"&&process.argv.length===4){
    console.log(JSON.stringify({mode:"--inspect",...inspect(process.argv[3])},null,2));
  }else{
    process.stderr.write("usage: node m4c13_manual_recovery.cjs --write|--check|--inspect EXACT_RESEARCH_RUN\n");
    process.exitCode=2;
  }
}
module.exports={classify,inspect,build,check,GUIDANCE};
