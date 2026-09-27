use super::{
    AG_PSD_CANDIDATE_NODE_MAJOR, AG_PSD_CANDIDATE_VERSION, AdapterErrorKind, CandidateDescriptor,
    CorpusError, PSD_TOOLS_REFERENCE_PYTHON, PSD_TOOLS_REFERENCE_VERSION, PsdCandidateAdapter,
    PsdFixture, candidate_adapters, load_corpus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

pub const BENCHMARK_SCHEMA_VERSION: &str = "1";
pub const BENCHMARK_SUITE_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkPlan {
    pub schema_version: String,
    pub fixture_id: String,
    pub warmup_iterations: u32,
    pub measured_iterations: u32,
    pub candidates: Vec<String>,
    pub layer_export_candidates: Vec<String>,
    pub ranking_allowed: bool,
    pub purpose: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkSuitePlan {
    pub schema_version: String,
    pub suite_id: String,
    pub corpus_path: String,
    pub ranking_allowed: bool,
    pub canonical_report_platform: String,
    pub plans: Vec<String>,
    pub purpose: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkSuiteReport {
    pub schema_version: String,
    pub suite_id: String,
    pub ranking_allowed: bool,
    pub canonical_report_platform: String,
    pub environment: BenchmarkEnvironment,
    pub reports: Vec<BenchmarkReport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub schema_version: String,
    pub fixture_id: String,
    pub fixture_path: String,
    pub fixture_bytes: u64,
    pub warmup_iterations: u32,
    pub measured_iterations: u32,
    pub ranking_allowed: bool,
    pub environment: BenchmarkEnvironment,
    pub candidates: Vec<CandidateBenchmarkReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkEnvironment {
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateBenchmarkReport {
    pub candidate: CandidateDescriptor,
    pub cold_inspect: DurationOperationReport,
    pub warm_parse: DurationOperationReport,
    pub layer_export_materialize: LayerExportOperationReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_rss_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkStatus {
    Measured,
    Unsupported,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DurationOperationReport {
    pub status: BenchmarkStatus,
    pub samples_ms: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<DurationSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerExportOperationReport {
    pub status: BenchmarkStatus,
    pub samples_ms: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<DurationSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exported_layer_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_rgba_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_checksum_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DurationSummary {
    pub min_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub mean_ms: f64,
}

#[derive(Debug, Deserialize)]
struct ExternalBenchmarkObservation {
    warm_parse_samples_ms: Vec<f64>,
    layer_export_samples_ms: Vec<f64>,
    exported_layer_count: usize,
    total_rgba_bytes: usize,
    export_checksum_sha256: String,
    peak_rss_bytes: u64,
}

pub fn load_benchmark_suite_plan(path: &Path) -> Result<BenchmarkSuitePlan, CorpusError> {
    let content = fs::read_to_string(path).map_err(|error| {
        CorpusError::new(format!(
            "failed to read PSD benchmark suite {}: {error}",
            path.display()
        ))
    })?;
    let suite = serde_json::from_str::<BenchmarkSuitePlan>(&content).map_err(|error| {
        CorpusError::new(format!(
            "failed to parse PSD benchmark suite {}: {error}",
            path.display()
        ))
    })?;
    validate_benchmark_suite_plan(&suite)?;
    Ok(suite)
}

pub fn validate_benchmark_suite_plan(suite: &BenchmarkSuitePlan) -> Result<(), CorpusError> {
    if suite.schema_version != BENCHMARK_SUITE_SCHEMA_VERSION {
        return Err(CorpusError::new(format!(
            "unsupported PSD benchmark suite schema {}; expected {}",
            suite.schema_version, BENCHMARK_SUITE_SCHEMA_VERSION
        )));
    }
    if suite.suite_id.trim().is_empty() {
        return Err(CorpusError::new("PSD benchmark suite_id must not be empty"));
    }
    if suite.corpus_path.trim().is_empty() {
        return Err(CorpusError::new(
            "PSD benchmark suite corpus_path must not be empty",
        ));
    }
    if suite.canonical_report_platform.trim().is_empty() {
        return Err(CorpusError::new(
            "PSD benchmark suite canonical_report_platform must not be empty",
        ));
    }
    if suite.ranking_allowed {
        return Err(CorpusError::new(
            "representative PSD benchmark suite v2 must not allow ranking",
        ));
    }
    if suite.plans.is_empty() {
        return Err(CorpusError::new(
            "PSD benchmark suite must contain at least one workload plan",
        ));
    }

    let mut unique = HashSet::new();
    for plan in &suite.plans {
        if plan.trim().is_empty() {
            return Err(CorpusError::new(
                "PSD benchmark suite plan path must not be empty",
            ));
        }
        if !unique.insert(plan.as_str()) {
            return Err(CorpusError::new(format!(
                "duplicate PSD benchmark suite plan: {plan}"
            )));
        }
    }

    Ok(())
}

pub fn run_benchmark_suite(path: &Path) -> Result<BenchmarkSuiteReport, CorpusError> {
    let suite = load_benchmark_suite_plan(path)?;
    let corpus_path = Path::new(&suite.corpus_path);
    let mut reports = Vec::with_capacity(suite.plans.len());

    for plan in &suite.plans {
        let report = run_benchmark(corpus_path, Path::new(plan))?;
        if report.ranking_allowed {
            return Err(CorpusError::new(format!(
                "PSD benchmark workload {plan} unexpectedly enables ranking"
            )));
        }
        reports.push(report);
    }

    Ok(BenchmarkSuiteReport {
        schema_version: BENCHMARK_SUITE_SCHEMA_VERSION.to_owned(),
        suite_id: suite.suite_id,
        ranking_allowed: suite.ranking_allowed,
        canonical_report_platform: suite.canonical_report_platform,
        environment: BenchmarkEnvironment {
            os: env::consts::OS.to_owned(),
            arch: env::consts::ARCH.to_owned(),
        },
        reports,
    })
}

pub fn load_benchmark_plan(path: &Path) -> Result<BenchmarkPlan, CorpusError> {
    let content = fs::read_to_string(path).map_err(|error| {
        CorpusError::new(format!(
            "failed to read PSD benchmark plan {}: {error}",
            path.display()
        ))
    })?;
    let plan = serde_json::from_str::<BenchmarkPlan>(&content).map_err(|error| {
        CorpusError::new(format!(
            "failed to parse PSD benchmark plan {}: {error}",
            path.display()
        ))
    })?;
    validate_benchmark_plan(&plan)?;
    Ok(plan)
}

pub fn validate_benchmark_plan(plan: &BenchmarkPlan) -> Result<(), CorpusError> {
    if plan.schema_version != BENCHMARK_SCHEMA_VERSION {
        return Err(CorpusError::new(format!(
            "unsupported PSD benchmark schema {}; expected {}",
            plan.schema_version, BENCHMARK_SCHEMA_VERSION
        )));
    }
    if plan.fixture_id.trim().is_empty() {
        return Err(CorpusError::new(
            "PSD benchmark fixture_id must not be empty",
        ));
    }
    if plan.warmup_iterations == 0 || plan.measured_iterations == 0 {
        return Err(CorpusError::new(
            "PSD benchmark warmup and measured iteration counts must be greater than zero",
        ));
    }
    if plan.measured_iterations > 100 {
        return Err(CorpusError::new(
            "PSD benchmark measured_iterations must not exceed 100 in the committed CI plan",
        ));
    }
    if plan.candidates.is_empty() {
        return Err(CorpusError::new(
            "PSD benchmark plan must contain at least one candidate",
        ));
    }
    if plan.ranking_allowed {
        return Err(CorpusError::new(
            "PSD benchmark plan v1 uses a smoke-sized fixture and must not allow ranking",
        ));
    }

    let registered = candidate_adapters()
        .into_iter()
        .map(|adapter| adapter.descriptor().id)
        .collect::<HashSet<_>>();
    let mut candidates = HashSet::new();
    for candidate in &plan.candidates {
        if !registered.contains(candidate) {
            return Err(CorpusError::new(format!(
                "unknown PSD benchmark candidate: {candidate}"
            )));
        }
        if !candidates.insert(candidate.as_str()) {
            return Err(CorpusError::new(format!(
                "duplicate PSD benchmark candidate: {candidate}"
            )));
        }
    }

    let mut export_candidates = HashSet::new();
    for candidate in &plan.layer_export_candidates {
        if !candidates.contains(candidate.as_str()) {
            return Err(CorpusError::new(format!(
                "layer-export candidate {candidate} is not in benchmark candidates"
            )));
        }
        if !export_candidates.insert(candidate.as_str()) {
            return Err(CorpusError::new(format!(
                "duplicate layer-export candidate: {candidate}"
            )));
        }
    }

    Ok(())
}

pub fn run_benchmark(corpus_path: &Path, plan_path: &Path) -> Result<BenchmarkReport, CorpusError> {
    let plan = load_benchmark_plan(plan_path)?;
    let corpus = load_corpus(corpus_path)?;
    let fixture = corpus
        .fixtures
        .iter()
        .find(|fixture| fixture.id == plan.fixture_id)
        .ok_or_else(|| {
            CorpusError::new(format!(
                "benchmark fixture {} is not present in {}",
                plan.fixture_id,
                corpus_path.display()
            ))
        })?;
    let root = corpus_path.parent().unwrap_or_else(|| Path::new("."));
    let input = root.join(&fixture.path);
    let fixture_bytes = fs::metadata(&input)
        .map_err(|error| {
            CorpusError::new(format!(
                "failed to stat PSD benchmark fixture {}: {error}",
                input.display()
            ))
        })?
        .len();

    let selected = plan.candidates.iter().cloned().collect::<HashSet<_>>();
    let export_selected = plan
        .layer_export_candidates
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let mut candidates = Vec::with_capacity(selected.len());

    for adapter in candidate_adapters() {
        let descriptor = adapter.descriptor();
        if !selected.contains(&descriptor.id) {
            continue;
        }

        let cold_inspect = run_cold_inspect(
            adapter.as_ref(),
            &input,
            fixture,
            plan.warmup_iterations,
            plan.measured_iterations,
        );

        let (warm_parse, layer_export_materialize, peak_rss_bytes) = match descriptor.id.as_str() {
            "rust-native" => {
                let warm_parse =
                    run_rawpsd_warm_parse(&input, plan.warmup_iterations, plan.measured_iterations);
                (
                    warm_parse,
                    unsupported_layer_export(
                        "rawpsd 0.2.2 exposes low-level image data but the M3 adapter does not yet provide a normalized RGBA layer-export contract",
                    ),
                    process_peak_rss_bytes(),
                )
            }
            "psd-tools" => match run_psd_tools_external(
                &input,
                plan.warmup_iterations,
                plan.measured_iterations,
            ) {
                Ok(result) => external_reports(result),
                Err((status, diagnostic)) => external_failure_reports(status, diagnostic),
            },
            "typescript-psd" => {
                match run_ag_psd_external(&input, plan.warmup_iterations, plan.measured_iterations)
                {
                    Ok(result) => external_reports(result),
                    Err((status, diagnostic)) => external_failure_reports(status, diagnostic),
                }
            }
            other => {
                let diagnostic = format!("benchmark implementation missing for {other}");
                external_failure_reports(BenchmarkStatus::Unsupported, diagnostic)
            }
        };

        candidates.push(CandidateBenchmarkReport {
            candidate: descriptor,
            cold_inspect,
            warm_parse,
            layer_export_materialize,
            peak_rss_bytes,
        });
    }

    let report = BenchmarkReport {
        schema_version: BENCHMARK_SCHEMA_VERSION.to_owned(),
        fixture_id: fixture.id.clone(),
        fixture_path: fixture.path.clone(),
        fixture_bytes,
        warmup_iterations: plan.warmup_iterations,
        measured_iterations: plan.measured_iterations,
        ranking_allowed: plan.ranking_allowed,
        environment: BenchmarkEnvironment {
            os: env::consts::OS.to_owned(),
            arch: env::consts::ARCH.to_owned(),
        },
        candidates,
    };
    validate_benchmark_report(&report, &plan, &export_selected)?;
    Ok(report)
}

fn run_cold_inspect(
    adapter: &dyn PsdCandidateAdapter,
    input: &Path,
    fixture: &PsdFixture,
    warmup_iterations: u32,
    measured_iterations: u32,
) -> DurationOperationReport {
    for _ in 0..warmup_iterations {
        match adapter.inspect(input, fixture) {
            Ok(observation) if observation.parse_success => {}
            Ok(_) => {
                return failed_duration(
                    BenchmarkStatus::Error,
                    "candidate rejected the benchmark fixture during cold-inspect warmup",
                );
            }
            Err(error) => {
                return failed_duration(
                    benchmark_status_from_adapter_error(error.kind),
                    error.message,
                );
            }
        }
    }

    let mut samples_ms = Vec::with_capacity(measured_iterations as usize);
    for _ in 0..measured_iterations {
        let started = Instant::now();
        let result = adapter.inspect(input, fixture);
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;

        match result {
            Ok(observation) if observation.parse_success => samples_ms.push(elapsed_ms),
            Ok(_) => {
                return failed_duration(
                    BenchmarkStatus::Error,
                    "candidate rejected the benchmark fixture during cold inspect",
                );
            }
            Err(error) => {
                return failed_duration(
                    benchmark_status_from_adapter_error(error.kind),
                    error.message,
                );
            }
        }
    }

    measured_duration(samples_ms)
}

fn benchmark_status_from_adapter_error(kind: AdapterErrorKind) -> BenchmarkStatus {
    match kind {
        AdapterErrorKind::Unavailable => BenchmarkStatus::Unavailable,
        AdapterErrorKind::Execution => BenchmarkStatus::Error,
    }
}

fn run_rawpsd_warm_parse(
    input: &Path,
    warmup_iterations: u32,
    measured_iterations: u32,
) -> DurationOperationReport {
    let bytes = match fs::read(input) {
        Ok(bytes) => bytes,
        Err(error) => {
            return failed_duration(
                BenchmarkStatus::Error,
                format!(
                    "failed to read rawpsd benchmark input {}: {error}",
                    input.display()
                ),
            );
        }
    };

    for _ in 0..warmup_iterations {
        if let Err(error) = rawpsd_parse_once(&bytes) {
            return failed_duration(BenchmarkStatus::Error, error);
        }
    }

    let mut samples_ms = Vec::with_capacity(measured_iterations as usize);
    for _ in 0..measured_iterations {
        let started = Instant::now();
        if let Err(error) = rawpsd_parse_once(&bytes) {
            return failed_duration(BenchmarkStatus::Error, error);
        }
        samples_ms.push(started.elapsed().as_secs_f64() * 1000.0);
    }

    measured_duration(samples_ms)
}

fn rawpsd_parse_once(bytes: &[u8]) -> Result<(), String> {
    let parsed = catch_unwind(AssertUnwindSafe(|| {
        rawpsd::parse_psd_metadata(bytes)?;
        rawpsd::parse_layer_records(bytes).map_err(|(_, error)| error)?;
        Ok::<(), String>(())
    }));

    match parsed {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(format!("rawpsd warm parse failed: {error}")),
        Err(_) => Err("rawpsd warm parse panicked".to_owned()),
    }
}

fn run_psd_tools_external(
    input: &Path,
    warmup_iterations: u32,
    measured_iterations: u32,
) -> Result<ExternalBenchmarkObservation, (BenchmarkStatus, String)> {
    let python = env::var_os("YU_PSD_TOOLS_PYTHON")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| OsString::from("python"));
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("adapters/psd_tools_benchmark.py");
    let warmup = warmup_iterations.to_string();
    let iterations = measured_iterations.to_string();

    run_external_command(
        &python,
        &script,
        &[
            "--expected-version",
            PSD_TOOLS_REFERENCE_VERSION,
            "--expected-python",
            PSD_TOOLS_REFERENCE_PYTHON,
            "--warmup",
            &warmup,
            "--iterations",
            &iterations,
        ],
        input,
        "psd-tools",
    )
}

fn run_ag_psd_external(
    input: &Path,
    warmup_iterations: u32,
    measured_iterations: u32,
) -> Result<ExternalBenchmarkObservation, (BenchmarkStatus, String)> {
    let node = env::var_os("YU_TYPESCRIPT_PSD_NODE")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| OsString::from("node"));
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("adapters/typescript/ag_psd_benchmark.cjs");
    let warmup = warmup_iterations.to_string();
    let iterations = measured_iterations.to_string();

    run_external_command(
        &node,
        &script,
        &[
            "--expected-version",
            AG_PSD_CANDIDATE_VERSION,
            "--expected-node-major",
            AG_PSD_CANDIDATE_NODE_MAJOR,
            "--warmup",
            &warmup,
            "--iterations",
            &iterations,
        ],
        input,
        "ag-psd",
    )
}

fn run_external_command(
    executable: &OsString,
    script: &Path,
    option_pairs: &[&str],
    input: &Path,
    label: &str,
) -> Result<ExternalBenchmarkObservation, (BenchmarkStatus, String)> {
    let output = Command::new(executable)
        .arg(script)
        .args(option_pairs)
        .arg(input)
        .output()
        .map_err(|error| {
            (
                BenchmarkStatus::Unavailable,
                format!("failed to start {label} benchmark runtime {executable:?}: {error}"),
            )
        })?;

    if !output.status.success() {
        let diagnostic = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let diagnostic = if diagnostic.is_empty() {
            format!("{label} benchmark adapter exited with {}", output.status)
        } else {
            diagnostic
        };
        return Err((
            if output.status.code() == Some(3) {
                BenchmarkStatus::Unavailable
            } else {
                BenchmarkStatus::Error
            },
            diagnostic,
        ));
    }

    serde_json::from_slice::<ExternalBenchmarkObservation>(&output.stdout).map_err(|error| {
        (
            BenchmarkStatus::Error,
            format!("invalid {label} benchmark JSON: {error}"),
        )
    })
}

fn external_reports(
    observation: ExternalBenchmarkObservation,
) -> (
    DurationOperationReport,
    LayerExportOperationReport,
    Option<u64>,
) {
    let warm_parse = measured_duration(observation.warm_parse_samples_ms);
    let samples_ms = observation.layer_export_samples_ms;
    let summary = summarize_samples(&samples_ms);

    (
        warm_parse,
        LayerExportOperationReport {
            status: BenchmarkStatus::Measured,
            samples_ms,
            summary,
            exported_layer_count: Some(observation.exported_layer_count),
            total_rgba_bytes: Some(observation.total_rgba_bytes),
            export_checksum_sha256: Some(observation.export_checksum_sha256),
            diagnostic: None,
        },
        Some(observation.peak_rss_bytes),
    )
}

fn external_failure_reports(
    status: BenchmarkStatus,
    diagnostic: String,
) -> (
    DurationOperationReport,
    LayerExportOperationReport,
    Option<u64>,
) {
    (
        failed_duration(status, diagnostic.clone()),
        LayerExportOperationReport {
            status,
            samples_ms: Vec::new(),
            summary: None,
            exported_layer_count: None,
            total_rgba_bytes: None,
            export_checksum_sha256: None,
            diagnostic: Some(diagnostic),
        },
        None,
    )
}

fn unsupported_layer_export(diagnostic: impl Into<String>) -> LayerExportOperationReport {
    LayerExportOperationReport {
        status: BenchmarkStatus::Unsupported,
        samples_ms: Vec::new(),
        summary: None,
        exported_layer_count: None,
        total_rgba_bytes: None,
        export_checksum_sha256: None,
        diagnostic: Some(diagnostic.into()),
    }
}

fn measured_duration(samples_ms: Vec<f64>) -> DurationOperationReport {
    let summary = summarize_samples(&samples_ms);
    DurationOperationReport {
        status: BenchmarkStatus::Measured,
        samples_ms,
        summary,
        diagnostic: None,
    }
}

fn failed_duration(
    status: BenchmarkStatus,
    diagnostic: impl Into<String>,
) -> DurationOperationReport {
    DurationOperationReport {
        status,
        samples_ms: Vec::new(),
        summary: None,
        diagnostic: Some(diagnostic.into()),
    }
}

fn summarize_samples(samples: &[f64]) -> Option<DurationSummary> {
    if samples.is_empty() {
        return None;
    }

    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let len = sorted.len();
    let min_ms = sorted[0];
    let max_ms = sorted[len - 1];
    let median_ms = if len.is_multiple_of(2) {
        (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0
    } else {
        sorted[len / 2]
    };
    let p95_index = ((len as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(len - 1);
    let mean_ms = sorted.iter().sum::<f64>() / len as f64;

    Some(DurationSummary {
        min_ms,
        median_ms,
        p95_ms: sorted[p95_index],
        max_ms,
        mean_ms,
    })
}

#[cfg(unix)]
fn process_peak_rss_bytes() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if result != 0 {
        return None;
    }
    let usage = unsafe { usage.assume_init() };
    let raw = u64::try_from(usage.ru_maxrss).ok()?;

    #[cfg(target_os = "macos")]
    {
        Some(raw)
    }
    #[cfg(not(target_os = "macos"))]
    {
        raw.checked_mul(1024)
    }
}

#[cfg(windows)]
fn process_peak_rss_bytes() -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut counters = std::mem::MaybeUninit::<PROCESS_MEMORY_COUNTERS>::zeroed();
    let size = u32::try_from(std::mem::size_of::<PROCESS_MEMORY_COUNTERS>()).ok()?;
    let handle = unsafe { GetCurrentProcess() };
    let ok = unsafe { GetProcessMemoryInfo(handle, counters.as_mut_ptr(), size) };
    if ok == 0 {
        return None;
    }
    let counters = unsafe { counters.assume_init() };
    u64::try_from(counters.PeakWorkingSetSize).ok()
}

#[cfg(not(any(unix, windows)))]
fn process_peak_rss_bytes() -> Option<u64> {
    None
}

fn validate_benchmark_report(
    report: &BenchmarkReport,
    plan: &BenchmarkPlan,
    export_selected: &HashSet<String>,
) -> Result<(), CorpusError> {
    if report.schema_version != BENCHMARK_SCHEMA_VERSION {
        return Err(CorpusError::new("PSD benchmark report schema drift"));
    }
    if report.ranking_allowed {
        return Err(CorpusError::new(
            "PSD benchmark smoke report must not enable ranking",
        ));
    }
    if report.candidates.len() != plan.candidates.len() {
        return Err(CorpusError::new(format!(
            "PSD benchmark report candidate count {} does not match plan {}",
            report.candidates.len(),
            plan.candidates.len()
        )));
    }

    let expected_samples = plan.measured_iterations as usize;
    for candidate in &report.candidates {
        validate_measured_duration(
            &candidate.candidate.id,
            "cold_inspect",
            &candidate.cold_inspect,
            expected_samples,
        )?;
        validate_measured_duration(
            &candidate.candidate.id,
            "warm_parse",
            &candidate.warm_parse,
            expected_samples,
        )?;
        if candidate.peak_rss_bytes.unwrap_or(0) == 0 {
            return Err(CorpusError::new(format!(
                "PSD benchmark candidate {} did not report peak RSS",
                candidate.candidate.id
            )));
        }

        if export_selected.contains(&candidate.candidate.id) {
            let export = &candidate.layer_export_materialize;
            if export.status != BenchmarkStatus::Measured
                || export.samples_ms.len() != expected_samples
                || export.exported_layer_count.unwrap_or(0) == 0
                || export.total_rgba_bytes.unwrap_or(0) == 0
                || export
                    .export_checksum_sha256
                    .as_deref()
                    .map(|value| value.len())
                    != Some(64)
            {
                return Err(CorpusError::new(format!(
                    "PSD benchmark candidate {} did not satisfy the layer-export contract",
                    candidate.candidate.id
                )));
            }
        }
    }

    Ok(())
}

fn validate_measured_duration(
    candidate_id: &str,
    operation: &str,
    report: &DurationOperationReport,
    expected_samples: usize,
) -> Result<(), CorpusError> {
    if report.status != BenchmarkStatus::Measured
        || report.samples_ms.len() != expected_samples
        || report.summary.is_none()
    {
        let diagnostic = report
            .diagnostic
            .as_deref()
            .unwrap_or("no diagnostic was returned");
        return Err(CorpusError::new(format!(
            "PSD benchmark candidate {candidate_id} did not produce {expected_samples} measured samples for {operation}: status={:?}; {diagnostic}",
            report.status
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::RAWPSD_CANDIDATE_VERSION;
    use super::*;
    use std::path::PathBuf;

    fn committed_corpus_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/psd/corpus.json")
    }

    fn committed_plan_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/data/psd-benchmark-plan-v1.json")
    }

    fn representative_suite_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/data/psd-benchmark-suite-v2.json")
    }

    fn controlled_report_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/data/psd-benchmark-report-v2.json")
    }

    #[test]
    fn committed_benchmark_plan_is_valid_and_non_ranking() {
        let plan = load_benchmark_plan(&committed_plan_path())
            .expect("committed benchmark plan should load");
        assert_eq!(plan.schema_version, BENCHMARK_SCHEMA_VERSION);
        assert_eq!(plan.fixture_id, "simple-pixel-layers-psd");
        assert!(!plan.ranking_allowed);
        assert_eq!(plan.candidates.len(), 3);
        assert_eq!(plan.layer_export_candidates.len(), 2);
    }

    #[test]
    fn representative_suite_is_valid_and_non_ranking() {
        let suite = load_benchmark_suite_plan(&representative_suite_path())
            .expect("representative benchmark suite should load");
        assert_eq!(suite.schema_version, BENCHMARK_SUITE_SCHEMA_VERSION);
        assert_eq!(suite.suite_id, "m3-representative-v2");
        assert!(!suite.ranking_allowed);
        assert_eq!(suite.canonical_report_platform, "ubuntu-latest");
        assert_eq!(suite.plans.len(), 7);
    }

    #[test]
    fn controlled_report_is_non_ranking_and_records_high_bit_divergence() {
        let content =
            fs::read_to_string(controlled_report_path()).expect("controlled report should exist");
        let value: serde_json::Value =
            serde_json::from_str(&content).expect("controlled report should parse");

        assert_eq!(value["schema_version"], "1");
        assert_eq!(value["report_id"], "m3-controlled-benchmark-v2");
        assert_eq!(value["ranking_allowed"], false);
        assert_eq!(value["source"]["workflow_run_id"], 36135069883_u64);

        let workloads = value["workloads"]
            .as_array()
            .expect("controlled report workloads should be an array");
        assert_eq!(workloads.len(), 7);

        for fixture_id in ["bench-high-bit-rgb", "bench-high-bit-psb"] {
            let workload = workloads
                .iter()
                .find(|workload| workload["fixture_id"] == fixture_id)
                .expect("high-bit workload should be present");
            let candidates = workload["candidates"]
                .as_array()
                .expect("candidate list should be an array");
            let psd_tools = candidates
                .iter()
                .find(|candidate| candidate["candidate_id"] == "psd-tools")
                .expect("psd-tools high-bit evidence should exist");
            let ag_psd = candidates
                .iter()
                .find(|candidate| candidate["candidate_id"] == "typescript-psd")
                .expect("ag-psd high-bit evidence should exist");

            assert_ne!(
                psd_tools["total_rgba_bytes"], ag_psd["total_rgba_bytes"],
                "high-bit export must remain explicitly non-equivalent until normalized"
            );
            assert_ne!(
                psd_tools["export_checksum_sha256"], ag_psd["export_checksum_sha256"],
                "high-bit export fingerprint divergence must remain explicit"
            );
        }
    }

    #[test]
    fn duration_summary_uses_all_samples() {
        let summary = summarize_samples(&[4.0, 1.0, 3.0, 2.0, 5.0]).expect("summary should exist");
        assert_eq!(summary.min_ms, 1.0);
        assert_eq!(summary.median_ms, 3.0);
        assert_eq!(summary.p95_ms, 5.0);
        assert_eq!(summary.max_ms, 5.0);
        assert_eq!(summary.mean_ms, 3.0);
    }

    #[test]
    fn rawpsd_warm_parse_smoke_uses_committed_fixture() {
        let corpus = load_corpus(&committed_corpus_path()).expect("corpus should load");
        let fixture = corpus
            .fixtures
            .iter()
            .find(|fixture| fixture.id == "simple-pixel-layers-psd")
            .expect("benchmark fixture should exist");
        let input = committed_corpus_path()
            .parent()
            .expect("corpus should have a parent")
            .join(&fixture.path);

        let report = run_rawpsd_warm_parse(&input, 1, 2);
        assert_eq!(report.status, BenchmarkStatus::Measured);
        assert_eq!(report.samples_ms.len(), 2);
        assert!(report.summary.is_some());
    }

    #[test]
    fn pinned_runtime_versions_remain_visible_to_benchmark_contract() {
        assert_eq!(PSD_TOOLS_REFERENCE_VERSION, "1.20.0");
        assert_eq!(PSD_TOOLS_REFERENCE_PYTHON, "3.12");
        assert_eq!(RAWPSD_CANDIDATE_VERSION, "0.2.2");
        assert_eq!(AG_PSD_CANDIDATE_VERSION, "31.0.2");
        assert_eq!(AG_PSD_CANDIDATE_NODE_MAJOR, "22");
    }
}
