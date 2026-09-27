use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub mod benchmark;

pub const CORPUS_SCHEMA_VERSION: &str = "1";
pub const REPORT_SCHEMA_VERSION: &str = "1";
pub const PSD_TOOLS_REFERENCE_VERSION: &str = "1.20.0";
pub const PSD_TOOLS_REFERENCE_PYTHON: &str = "3.12";
pub const RAWPSD_CANDIDATE_VERSION: &str = "0.2.2";
pub const AG_PSD_CANDIDATE_VERSION: &str = "31.0.2";
pub const AG_PSD_CANDIDATE_NODE_MAJOR: &str = "22";
pub const COMPARISON_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdCorpus {
    pub schema_version: String,
    pub fixtures: Vec<PsdFixture>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdFixture {
    pub id: String,
    pub path: String,
    pub format: PsdFormat,
    pub features: Vec<FixtureFeature>,
    pub expected: FixtureExpectation,
    pub provenance: FixtureProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsdFormat {
    Psd,
    Psb,
}

impl PsdFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Psd => "psd",
            Self::Psb => "psb",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureFeature {
    SimplePixelLayers,
    NestedGroups,
    DuplicateLayerNames,
    TextLayers,
    Masks,
    BlendModes,
    Effects,
    SmartObjectMetadata,
    HighBitDepth,
    Malformed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureExpectation {
    pub parse: ParseExpectation,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub layer_count: Option<usize>,
    #[serde(default)]
    pub minimum_tree_depth: Option<usize>,
    #[serde(default)]
    pub layer_names: Option<Vec<String>>,
    #[serde(default)]
    pub text_layer_count: Option<usize>,
    #[serde(default)]
    pub pixel_mask_layer_count: Option<usize>,
    #[serde(default)]
    pub vector_mask_layer_count: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseExpectation {
    Accept,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureProvenance {
    pub kind: FixtureProvenanceKind,
    pub source: String,
    #[serde(default)]
    pub license: Option<String>,
    pub redistributable: bool,
    #[serde(default)]
    pub upstream_commit: Option<String>,
    #[serde(default)]
    pub upstream_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureProvenanceKind {
    Synthetic,
    Redistributable,
    LocalOnly,
}

#[derive(Debug)]
pub struct CorpusError {
    message: String,
}

impl CorpusError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CorpusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CorpusError {}

pub fn load_corpus(path: &Path) -> Result<PsdCorpus, CorpusError> {
    let content = fs::read_to_string(path).map_err(|error| {
        CorpusError::new(format!(
            "failed to read PSD corpus {}: {error}",
            path.display()
        ))
    })?;
    let corpus = serde_json::from_str::<PsdCorpus>(&content).map_err(|error| {
        CorpusError::new(format!(
            "failed to parse PSD corpus {}: {error}",
            path.display()
        ))
    })?;
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    validate_corpus(&corpus, root)?;
    Ok(corpus)
}

pub fn validate_corpus(corpus: &PsdCorpus, root: &Path) -> Result<(), CorpusError> {
    if corpus.schema_version != CORPUS_SCHEMA_VERSION {
        return Err(CorpusError::new(format!(
            "unsupported PSD corpus schema {}; expected {}",
            corpus.schema_version, CORPUS_SCHEMA_VERSION
        )));
    }

    if corpus.fixtures.is_empty() {
        return Err(CorpusError::new(
            "PSD corpus must contain at least one fixture",
        ));
    }

    let mut ids = HashSet::new();
    for fixture in &corpus.fixtures {
        if fixture.id.trim().is_empty() {
            return Err(CorpusError::new("PSD fixture ID must not be empty"));
        }
        if !ids.insert(fixture.id.as_str()) {
            return Err(CorpusError::new(format!(
                "duplicate PSD fixture ID: {}",
                fixture.id
            )));
        }
        if fixture.features.is_empty() {
            return Err(CorpusError::new(format!(
                "PSD fixture {} must declare at least one feature",
                fixture.id
            )));
        }

        let relative = Path::new(&fixture.path);
        if fixture.path.trim().is_empty()
            || relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
        {
            return Err(CorpusError::new(format!(
                "PSD fixture {} uses an unsafe path: {}",
                fixture.id, fixture.path
            )));
        }

        let extension = relative
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !extension.eq_ignore_ascii_case(fixture.format.extension()) {
            return Err(CorpusError::new(format!(
                "PSD fixture {} format/path mismatch: expected .{}",
                fixture.id,
                fixture.format.extension()
            )));
        }

        let resolved = root.join(relative);
        if !resolved.is_file() {
            return Err(CorpusError::new(format!(
                "PSD fixture {} is missing: {}",
                fixture.id,
                resolved.display()
            )));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateComparisonSnapshot {
    pub schema_version: String,
    pub observation_date: String,
    pub corpus: ComparisonCorpus,
    pub decision_state: String,
    pub benchmark: ComparisonBenchmark,
    pub candidates: Vec<ComparisonCandidate>,
    pub decision_gaps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonCorpus {
    pub path: String,
    pub schema_version: String,
    pub fixture_count: usize,
    pub covered_features: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonBenchmark {
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonCandidate {
    pub id: String,
    pub engine: String,
    pub version: String,
    pub runtime: String,
    pub runtime_pin: String,
    pub license: String,
    pub repository: String,
    pub latest_observed_commit: String,
    pub latest_observed_commit_date: String,
    pub distribution: ComparisonDistribution,
    pub corpus_result: ComparisonCorpusResult,
    pub verified: serde_json::Value,
    pub upstream_api_evidence: serde_json::Value,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonDistribution {
    pub model: String,
    pub core_dependency: bool,
    pub direct_dependencies: Vec<String>,
    pub optional_render_dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonCorpusResult {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errors: usize,
    pub failed_fixtures: Vec<String>,
}

pub fn load_candidate_comparison(path: &Path) -> Result<CandidateComparisonSnapshot, CorpusError> {
    let content = fs::read_to_string(path).map_err(|error| {
        CorpusError::new(format!(
            "failed to read PSD candidate comparison {}: {error}",
            path.display()
        ))
    })?;
    let comparison =
        serde_json::from_str::<CandidateComparisonSnapshot>(&content).map_err(|error| {
            CorpusError::new(format!(
                "failed to parse PSD candidate comparison {}: {error}",
                path.display()
            ))
        })?;
    validate_candidate_comparison(&comparison)?;
    Ok(comparison)
}

pub fn validate_candidate_comparison(
    comparison: &CandidateComparisonSnapshot,
) -> Result<(), CorpusError> {
    if comparison.schema_version != COMPARISON_SCHEMA_VERSION {
        return Err(CorpusError::new(format!(
            "unsupported PSD comparison schema {}; expected {}",
            comparison.schema_version, COMPARISON_SCHEMA_VERSION
        )));
    }
    if comparison.corpus.schema_version != CORPUS_SCHEMA_VERSION {
        return Err(CorpusError::new(format!(
            "comparison corpus schema {} does not match harness schema {}",
            comparison.corpus.schema_version, CORPUS_SCHEMA_VERSION
        )));
    }
    if comparison.corpus.fixture_count == 0 {
        return Err(CorpusError::new(
            "PSD candidate comparison must reference at least one fixture",
        ));
    }
    if comparison.decision_state != "evidence_only" {
        return Err(CorpusError::new(
            "PR #15 comparison must remain evidence_only",
        ));
    }
    if comparison.benchmark.status != "not_measured" {
        return Err(CorpusError::new(
            "PR #15 must not claim a measured benchmark",
        ));
    }
    if comparison.candidates.len() != 3 {
        return Err(CorpusError::new(format!(
            "PSD candidate comparison must contain exactly three candidates; found {}",
            comparison.candidates.len()
        )));
    }

    let mut ids = HashSet::new();
    for candidate in &comparison.candidates {
        if !ids.insert(candidate.id.as_str()) {
            return Err(CorpusError::new(format!(
                "duplicate PSD comparison candidate ID: {}",
                candidate.id
            )));
        }
        if candidate.distribution.core_dependency {
            return Err(CorpusError::new(format!(
                "PSD spike candidate {} must not be marked as a YuTool core dependency",
                candidate.id
            )));
        }

        let total = candidate.corpus_result.passed
            + candidate.corpus_result.failed
            + candidate.corpus_result.skipped
            + candidate.corpus_result.errors;
        if total != comparison.corpus.fixture_count {
            return Err(CorpusError::new(format!(
                "PSD comparison candidate {} result total {} does not match fixture count {}",
                candidate.id, total, comparison.corpus.fixture_count
            )));
        }
        if candidate.corpus_result.failed_fixtures.len() != candidate.corpus_result.failed {
            return Err(CorpusError::new(format!(
                "PSD comparison candidate {} failed fixture list does not match failed count",
                candidate.id
            )));
        }
    }

    let expected_ids = ["psd-tools", "rust-native", "typescript-psd"];
    if expected_ids.iter().any(|id| !ids.contains(id)) {
        return Err(CorpusError::new(
            "PSD candidate comparison is missing an M3 candidate",
        ));
    }

    validate_comparison_baseline(
        comparison,
        "psd-tools",
        PSD_TOOLS_REFERENCE_VERSION,
        7,
        0,
        &[],
    )?;
    validate_comparison_baseline(
        comparison,
        "rust-native",
        RAWPSD_CANDIDATE_VERSION,
        4,
        3,
        &["layer-masks", "simple-pixel-layers-psb", "text-layer"],
    )?;
    validate_comparison_baseline(
        comparison,
        "typescript-psd",
        AG_PSD_CANDIDATE_VERSION,
        7,
        0,
        &[],
    )?;

    if comparison.decision_gaps.is_empty() {
        return Err(CorpusError::new(
            "PSD candidate comparison must record remaining decision gaps",
        ));
    }

    Ok(())
}

fn validate_comparison_baseline(
    comparison: &CandidateComparisonSnapshot,
    id: &str,
    version: &str,
    passed: usize,
    failed: usize,
    expected_failed_fixtures: &[&str],
) -> Result<(), CorpusError> {
    let candidate = comparison
        .candidates
        .iter()
        .find(|candidate| candidate.id == id)
        .ok_or_else(|| CorpusError::new(format!("missing PSD comparison candidate: {id}")))?;

    if candidate.version != version {
        return Err(CorpusError::new(format!(
            "PSD comparison candidate {id} version {} does not match pinned version {version}",
            candidate.version
        )));
    }
    if candidate.corpus_result.passed != passed || candidate.corpus_result.failed != failed {
        return Err(CorpusError::new(format!(
            "PSD comparison candidate {id} baseline changed: expected {passed} passed / {failed} failed"
        )));
    }
    if candidate.corpus_result.skipped != 0 || candidate.corpus_result.errors != 0 {
        return Err(CorpusError::new(format!(
            "PSD comparison candidate {id} must have zero skipped/errors in corpus v1 baseline"
        )));
    }

    let mut actual_failed = candidate.corpus_result.failed_fixtures.clone();
    actual_failed.sort();
    let mut expected_failed = expected_failed_fixtures
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    expected_failed.sort();

    if actual_failed != expected_failed {
        return Err(CorpusError::new(format!(
            "PSD comparison candidate {id} failed fixture baseline changed"
        )));
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateDescriptor {
    pub id: String,
    pub display_name: String,
    pub runtime: CandidateRuntime,
    pub status: CandidateStatus,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateRuntime {
    RustNative,
    Python,
    TypeScriptNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    Skeleton,
    Wired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AdapterObservation {
    pub parse_success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_tree_depth: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_layer_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_mask_layer_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_mask_layer_count: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterErrorKind {
    Unavailable,
    Execution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterError {
    pub kind: AdapterErrorKind,
    pub message: String,
}

impl AdapterError {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            kind: AdapterErrorKind::Unavailable,
            message: message.into(),
        }
    }

    pub fn execution(message: impl Into<String>) -> Self {
        Self {
            kind: AdapterErrorKind::Execution,
            message: message.into(),
        }
    }
}

pub trait PsdCandidateAdapter {
    fn descriptor(&self) -> CandidateDescriptor;

    fn inspect(
        &self,
        input: &Path,
        fixture: &PsdFixture,
    ) -> Result<AdapterObservation, AdapterError>;
}

#[derive(Debug, Clone, Copy, Default)]
struct RawPsdCandidateAdapter;

impl RawPsdCandidateAdapter {
    fn parse(input: &Path) -> Result<AdapterObservation, AdapterError> {
        let bytes = fs::read(input).map_err(|error| {
            AdapterError::execution(format!(
                "failed to read rawpsd candidate input {}: {error}",
                input.display()
            ))
        })?;

        let parsed = catch_unwind(AssertUnwindSafe(|| {
            let metadata = rawpsd::parse_psd_metadata(&bytes)?;
            let layers = rawpsd::parse_layer_records(&bytes).map_err(|(_, error)| error)?;
            Ok::<_, String>((metadata, layers))
        }));

        let (metadata, layers) = match parsed {
            Ok(Ok(parsed)) => parsed,
            Ok(Err(_)) | Err(_) => {
                return Ok(AdapterObservation {
                    parse_success: false,
                    ..AdapterObservation::default()
                });
            }
        };

        let mut logical_layer_count = 0usize;
        let mut maximum_tree_depth = 0usize;
        let mut current_depth = 1usize;
        let mut layer_names = Vec::new();
        let mut pixel_mask_layer_count = 0usize;

        for layer in &layers {
            if layer.group_closer {
                current_depth = current_depth.saturating_add(1);
                continue;
            }

            if layer.group_opener {
                let group_depth = current_depth.saturating_sub(1).max(1);
                logical_layer_count += 1;
                maximum_tree_depth = maximum_tree_depth.max(group_depth);
                layer_names.push(layer.name.clone());
                if layer.mask_channel_count > 0 {
                    pixel_mask_layer_count += 1;
                }
                current_depth = group_depth;
                continue;
            }

            logical_layer_count += 1;
            maximum_tree_depth = maximum_tree_depth.max(current_depth);
            layer_names.push(layer.name.clone());
            if layer.mask_channel_count > 0 {
                pixel_mask_layer_count += 1;
            }
        }

        Ok(AdapterObservation {
            parse_success: true,
            width: Some(metadata.width),
            height: Some(metadata.height),
            layer_count: Some(logical_layer_count),
            maximum_tree_depth: Some(maximum_tree_depth),
            layer_names: Some(layer_names),
            text_layer_count: None,
            pixel_mask_layer_count: Some(pixel_mask_layer_count),
            vector_mask_layer_count: None,
        })
    }
}

impl PsdCandidateAdapter for RawPsdCandidateAdapter {
    fn descriptor(&self) -> CandidateDescriptor {
        CandidateDescriptor {
            id: "rust-native".to_owned(),
            display_name: format!("rawpsd {RAWPSD_CANDIDATE_VERSION} candidate"),
            runtime: CandidateRuntime::RustNative,
            status: CandidateStatus::Wired,
            notes: "Rust-native metadata candidate. rawpsd 0.2.2 supports PSD but not PSB and does not currently expose normalized text-layer or vector-mask semantics.".to_owned(),
        }
    }

    fn inspect(
        &self,
        input: &Path,
        _fixture: &PsdFixture,
    ) -> Result<AdapterObservation, AdapterError> {
        Self::parse(input)
    }
}

#[derive(Debug, Clone)]
struct PsdToolsReferenceAdapter {
    python: OsString,
}

impl Default for PsdToolsReferenceAdapter {
    fn default() -> Self {
        let python = env::var_os("YU_PSD_TOOLS_PYTHON")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| OsString::from("python"));
        Self { python }
    }
}

impl PsdToolsReferenceAdapter {
    #[cfg(test)]
    fn with_python(python: impl Into<OsString>) -> Self {
        Self {
            python: python.into(),
        }
    }

    fn script_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("adapters/psd_tools_reference.py")
    }
}

impl PsdCandidateAdapter for PsdToolsReferenceAdapter {
    fn descriptor(&self) -> CandidateDescriptor {
        CandidateDescriptor {
            id: "psd-tools".to_owned(),
            display_name: format!("psd-tools {PSD_TOOLS_REFERENCE_VERSION} reference"),
            runtime: CandidateRuntime::Python,
            status: CandidateStatus::Wired,
            notes: format!(
                "Reference adapter pinned to Python {PSD_TOOLS_REFERENCE_PYTHON} + psd-tools {PSD_TOOLS_REFERENCE_VERSION}; set YU_PSD_TOOLS_PYTHON to select the interpreter."
            ),
        }
    }

    fn inspect(
        &self,
        input: &Path,
        _fixture: &PsdFixture,
    ) -> Result<AdapterObservation, AdapterError> {
        let output = Command::new(&self.python)
            .arg(Self::script_path())
            .arg("--expected-version")
            .arg(PSD_TOOLS_REFERENCE_VERSION)
            .arg("--expected-python")
            .arg(PSD_TOOLS_REFERENCE_PYTHON)
            .arg(input)
            .output()
            .map_err(|error| {
                AdapterError::unavailable(format!(
                    "failed to start psd-tools reference Python {:?}: {error}",
                    self.python
                ))
            })?;

        if !output.status.success() {
            let diagnostic = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let diagnostic = if diagnostic.is_empty() {
                format!("psd-tools reference adapter exited with {}", output.status)
            } else {
                diagnostic
            };

            if output.status.code() == Some(3) {
                return Err(AdapterError::unavailable(diagnostic));
            }
            return Err(AdapterError::execution(diagnostic));
        }

        serde_json::from_slice::<AdapterObservation>(&output.stdout).map_err(|error| {
            AdapterError::execution(format!("invalid psd-tools reference adapter JSON: {error}"))
        })
    }
}

#[derive(Debug, Clone)]
struct AgPsdCandidateAdapter {
    node: OsString,
}

impl Default for AgPsdCandidateAdapter {
    fn default() -> Self {
        let node = env::var_os("YU_TYPESCRIPT_PSD_NODE")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| OsString::from("node"));
        Self { node }
    }
}

impl AgPsdCandidateAdapter {
    #[cfg(test)]
    fn with_node(node: impl Into<OsString>) -> Self {
        Self { node: node.into() }
    }

    fn script_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("adapters/typescript/ag_psd_candidate.cjs")
    }
}

impl PsdCandidateAdapter for AgPsdCandidateAdapter {
    fn descriptor(&self) -> CandidateDescriptor {
        CandidateDescriptor {
            id: "typescript-psd".to_owned(),
            display_name: format!("ag-psd {AG_PSD_CANDIDATE_VERSION} candidate"),
            runtime: CandidateRuntime::TypeScriptNode,
            status: CandidateStatus::Wired,
            notes: format!(
                "TypeScript/Node candidate pinned to Node.js {AG_PSD_CANDIDATE_NODE_MAJOR} + ag-psd {AG_PSD_CANDIDATE_VERSION}; set YU_TYPESCRIPT_PSD_NODE to select the Node executable."
            ),
        }
    }

    fn inspect(
        &self,
        input: &Path,
        _fixture: &PsdFixture,
    ) -> Result<AdapterObservation, AdapterError> {
        let output = Command::new(&self.node)
            .arg(Self::script_path())
            .arg("--expected-version")
            .arg(AG_PSD_CANDIDATE_VERSION)
            .arg("--expected-node-major")
            .arg(AG_PSD_CANDIDATE_NODE_MAJOR)
            .arg(input)
            .output()
            .map_err(|error| {
                AdapterError::unavailable(format!(
                    "failed to start ag-psd candidate Node.js {:?}: {error}",
                    self.node
                ))
            })?;

        if !output.status.success() {
            let diagnostic = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let diagnostic = if diagnostic.is_empty() {
                format!("ag-psd candidate adapter exited with {}", output.status)
            } else {
                diagnostic
            };

            if output.status.code() == Some(3) {
                return Err(AdapterError::unavailable(diagnostic));
            }
            return Err(AdapterError::execution(diagnostic));
        }

        serde_json::from_slice::<AdapterObservation>(&output.stdout).map_err(|error| {
            AdapterError::execution(format!("invalid ag-psd candidate adapter JSON: {error}"))
        })
    }
}

pub fn candidate_adapters() -> Vec<Box<dyn PsdCandidateAdapter>> {
    vec![
        Box::new(RawPsdCandidateAdapter),
        Box::new(PsdToolsReferenceAdapter::default()),
        Box::new(AgPsdCandidateAdapter::default()),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateReport {
    pub schema_version: String,
    pub candidate: CandidateDescriptor,
    pub fixtures: Vec<FixtureReport>,
    pub summary: ReportSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureReport {
    pub fixture_id: String,
    pub status: FixtureStatus,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<AdapterObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureStatus {
    Passed,
    Failed,
    Skipped,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReportSummary {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errors: usize,
}

pub fn run_candidate(
    corpus_path: &Path,
    adapter: &dyn PsdCandidateAdapter,
) -> Result<CandidateReport, CorpusError> {
    let corpus = load_corpus(corpus_path)?;
    let root = corpus_path.parent().unwrap_or_else(|| Path::new("."));
    let mut fixtures = Vec::with_capacity(corpus.fixtures.len());

    for fixture in &corpus.fixtures {
        let input = root.join(&fixture.path);
        let started = Instant::now();
        let result = adapter.inspect(&input, fixture);
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

        let report = match result {
            Ok(observation) => {
                let issues = compare_observation(fixture, &observation);
                if issues.is_empty() {
                    FixtureReport {
                        fixture_id: fixture.id.clone(),
                        status: FixtureStatus::Passed,
                        duration_ms,
                        observation: Some(observation),
                        diagnostic: None,
                    }
                } else {
                    FixtureReport {
                        fixture_id: fixture.id.clone(),
                        status: FixtureStatus::Failed,
                        duration_ms,
                        observation: Some(observation),
                        diagnostic: Some(issues.join("; ")),
                    }
                }
            }
            Err(error) => FixtureReport {
                fixture_id: fixture.id.clone(),
                status: match error.kind {
                    AdapterErrorKind::Unavailable => FixtureStatus::Skipped,
                    AdapterErrorKind::Execution => FixtureStatus::Error,
                },
                duration_ms,
                observation: None,
                diagnostic: Some(error.message),
            },
        };

        fixtures.push(report);
    }

    let summary = summarize(&fixtures);
    Ok(CandidateReport {
        schema_version: REPORT_SCHEMA_VERSION.to_owned(),
        candidate: adapter.descriptor(),
        fixtures,
        summary,
    })
}

fn compare_observation(fixture: &PsdFixture, observation: &AdapterObservation) -> Vec<String> {
    let expected_parse = matches!(fixture.expected.parse, ParseExpectation::Accept);
    if observation.parse_success != expected_parse {
        return vec![format!(
            "parse expectation mismatch: expected {}, observed {}",
            fixture.expected.parse.as_str(),
            if observation.parse_success {
                "accept"
            } else {
                "reject"
            }
        )];
    }

    if !observation.parse_success {
        return Vec::new();
    }

    let mut issues = Vec::new();
    compare_optional(
        "width",
        fixture.expected.width,
        observation.width,
        &mut issues,
    );
    compare_optional(
        "height",
        fixture.expected.height,
        observation.height,
        &mut issues,
    );
    compare_optional(
        "layer_count",
        fixture.expected.layer_count,
        observation.layer_count,
        &mut issues,
    );
    compare_minimum_optional(
        "minimum_tree_depth",
        fixture.expected.minimum_tree_depth,
        observation.maximum_tree_depth,
        &mut issues,
    );
    compare_layer_names(
        fixture.expected.layer_names.as_deref(),
        observation.layer_names.as_deref(),
        &mut issues,
    );
    compare_optional(
        "text_layer_count",
        fixture.expected.text_layer_count,
        observation.text_layer_count,
        &mut issues,
    );
    compare_optional(
        "pixel_mask_layer_count",
        fixture.expected.pixel_mask_layer_count,
        observation.pixel_mask_layer_count,
        &mut issues,
    );
    compare_optional(
        "vector_mask_layer_count",
        fixture.expected.vector_mask_layer_count,
        observation.vector_mask_layer_count,
        &mut issues,
    );

    issues
}

impl ParseExpectation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Reject => "reject",
        }
    }
}

fn compare_optional<T>(
    field: &str,
    expected: Option<T>,
    observed: Option<T>,
    issues: &mut Vec<String>,
) where
    T: Copy + PartialEq + fmt::Display,
{
    let Some(expected) = expected else {
        return;
    };
    match observed {
        Some(observed) if observed == expected => {}
        Some(observed) => issues.push(format!(
            "{field} mismatch: expected {expected}, observed {observed}"
        )),
        None => issues.push(format!("{field} missing: expected {expected}")),
    }
}

fn compare_minimum_optional(
    field: &str,
    expected_minimum: Option<usize>,
    observed: Option<usize>,
    issues: &mut Vec<String>,
) {
    let Some(expected_minimum) = expected_minimum else {
        return;
    };
    match observed {
        Some(observed) if observed >= expected_minimum => {}
        Some(observed) => issues.push(format!(
            "{field} below minimum: expected at least {expected_minimum}, observed {observed}"
        )),
        None => issues.push(format!(
            "{field} missing: expected at least {expected_minimum}"
        )),
    }
}

fn compare_layer_names(
    expected: Option<&[String]>,
    observed: Option<&[String]>,
    issues: &mut Vec<String>,
) {
    let Some(expected) = expected else {
        return;
    };
    let Some(observed) = observed else {
        issues.push("layer_names missing".to_owned());
        return;
    };

    let mut expected = expected.to_vec();
    let mut observed = observed.to_vec();
    expected.sort();
    observed.sort();

    if expected != observed {
        issues.push(format!(
            "layer_names mismatch: expected {:?}, observed {:?}",
            expected, observed
        ));
    }
}

fn summarize(fixtures: &[FixtureReport]) -> ReportSummary {
    let mut summary = ReportSummary::default();
    for fixture in fixtures {
        match fixture.status {
            FixtureStatus::Passed => summary.passed += 1,
            FixtureStatus::Failed => summary.failed += 1,
            FixtureStatus::Skipped => summary.skipped += 1,
            FixtureStatus::Error => summary.errors += 1,
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn committed_corpus_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/psd/corpus.json")
    }

    fn committed_comparison_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/data/psd-candidate-comparison-v1.json")
    }

    fn committed_strategy_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/data/psd-engine-strategy-v1.json")
    }

    struct ExpectedObservationAdapter;

    impl PsdCandidateAdapter for ExpectedObservationAdapter {
        fn descriptor(&self) -> CandidateDescriptor {
            CandidateDescriptor {
                id: "test-expected-observation".to_owned(),
                display_name: "Test expected-observation adapter".to_owned(),
                runtime: CandidateRuntime::RustNative,
                status: CandidateStatus::Wired,
                notes: "Test-only adapter that mirrors fixture expectations.".to_owned(),
            }
        }

        fn inspect(
            &self,
            _input: &Path,
            fixture: &PsdFixture,
        ) -> Result<AdapterObservation, AdapterError> {
            Ok(AdapterObservation {
                parse_success: matches!(fixture.expected.parse, ParseExpectation::Accept),
                width: fixture.expected.width,
                height: fixture.expected.height,
                layer_count: fixture.expected.layer_count,
                maximum_tree_depth: fixture.expected.minimum_tree_depth,
                layer_names: fixture.expected.layer_names.clone(),
                text_layer_count: fixture.expected.text_layer_count,
                pixel_mask_layer_count: fixture.expected.pixel_mask_layer_count,
                vector_mask_layer_count: fixture.expected.vector_mask_layer_count,
            })
        }
    }

    #[test]
    fn committed_comparison_is_valid_and_frozen() {
        let comparison = load_candidate_comparison(&committed_comparison_path())
            .expect("committed PSD candidate comparison should be valid");

        assert_eq!(comparison.schema_version, COMPARISON_SCHEMA_VERSION);
        assert_eq!(comparison.corpus.fixture_count, 7);
        assert_eq!(comparison.decision_state, "evidence_only");
        assert_eq!(comparison.benchmark.status, "not_measured");
        assert_eq!(comparison.candidates.len(), 3);
    }

    #[test]
    fn comparison_snapshot_versions_match_registered_candidates() {
        let comparison = load_candidate_comparison(&committed_comparison_path())
            .expect("committed PSD candidate comparison should be valid");
        let descriptors = candidate_adapters()
            .into_iter()
            .map(|adapter| adapter.descriptor())
            .collect::<Vec<_>>();

        for candidate in &comparison.candidates {
            let descriptor = descriptors
                .iter()
                .find(|descriptor| descriptor.id == candidate.id)
                .expect("comparison candidate should be registered");
            assert!(
                descriptor.display_name.contains(&candidate.version),
                "descriptor version drift for {}",
                candidate.id
            );
        }
    }

    #[test]
    fn committed_psd_engine_strategy_is_accepted_and_guarded() {
        let content =
            fs::read_to_string(committed_strategy_path()).expect("PSD strategy should exist");
        let strategy: serde_json::Value =
            serde_json::from_str(&content).expect("PSD strategy should parse");

        assert_eq!(strategy["schema_version"], "1");
        assert_eq!(strategy["status"], "accepted");
        assert_eq!(strategy["milestone"], "M3");

        let primary = &strategy["primary_engine"];
        assert_eq!(primary["engine_id"], "ag-psd");
        assert_eq!(primary["candidate_id"], "typescript-psd");
        assert_eq!(primary["version"], AG_PSD_CANDIDATE_VERSION);
        assert_eq!(primary["provider_class"], "managed");
        assert_eq!(primary["runtime"], "node");
        assert_eq!(primary["runtime_version"], AG_PSD_CANDIDATE_NODE_MAJOR);
        assert_eq!(primary["auto_install"], false);
        assert_eq!(primary["auto_activate"], false);

        let compatibility = &strategy["compatibility_engine"];
        assert_eq!(compatibility["engine_id"], "psd-tools");
        assert_eq!(compatibility["version"], PSD_TOOLS_REFERENCE_VERSION);
        assert_eq!(compatibility["runtime_version"], PSD_TOOLS_REFERENCE_PYTHON);
        assert_eq!(compatibility["production_default"], false);
        assert_eq!(compatibility["automatic_fallback"], false);

        let deferred = &strategy["deferred_candidate"];
        assert_eq!(deferred["engine_id"], "rawpsd");
        assert_eq!(deferred["version"], RAWPSD_CANDIDATE_VERSION);
        assert_eq!(deferred["production_default"], false);

        assert_eq!(
            strategy["resolution_policy"]["automatic_fallback_between_psd_engines"],
            false
        );
        assert_eq!(
            strategy["resolution_policy"]["selected_engine_visible_in_results"],
            true
        );

        let export = &strategy["v0_1_contract"]["layer_export"];
        assert_eq!(export["status"], "partial");
        assert_eq!(export["normalized_pixel_format"], "rgba8");
        assert_eq!(export["output_container"], "png");
        assert_eq!(export["full_render_fidelity_guarantee"], false);
        assert_eq!(
            strategy["high_bit_depth_policy"]["layer_export"],
            "unsupported_in_v0_1"
        );
        assert_eq!(strategy["v0_1_contract"]["render"]["status"], "deferred");
        assert_eq!(strategy["v0_1_contract"]["mutation"]["status"], "deferred");
    }

    #[test]
    fn committed_corpus_is_valid() {
        let corpus =
            load_corpus(&committed_corpus_path()).expect("committed corpus should be valid");
        assert_eq!(corpus.schema_version, CORPUS_SCHEMA_VERSION);
        assert_eq!(corpus.fixtures.len(), 7);
    }

    #[test]
    fn committed_expectations_can_be_compared() {
        let report = run_candidate(&committed_corpus_path(), &ExpectedObservationAdapter)
            .expect("test adapter should run");
        assert_eq!(report.summary.passed, 7);
        assert_eq!(report.summary.failed, 0);
        assert_eq!(report.summary.skipped, 0);
        assert_eq!(report.summary.errors, 0);
    }

    #[test]
    fn committed_corpus_covers_initial_m3_matrix() {
        let corpus = load_corpus(&committed_corpus_path()).expect("committed corpus should load");
        assert!(
            corpus
                .fixtures
                .iter()
                .any(|fixture| fixture.format == PsdFormat::Psb)
        );

        for feature in [
            FixtureFeature::SimplePixelLayers,
            FixtureFeature::NestedGroups,
            FixtureFeature::DuplicateLayerNames,
            FixtureFeature::TextLayers,
            FixtureFeature::Masks,
            FixtureFeature::Malformed,
        ] {
            assert!(
                corpus
                    .fixtures
                    .iter()
                    .any(|fixture| fixture.features.contains(&feature)),
                "missing fixture feature: {feature:?}"
            );
        }
    }

    #[test]
    fn all_m3_candidate_slots_are_wired() {
        let descriptors = candidate_adapters()
            .into_iter()
            .map(|adapter| adapter.descriptor())
            .collect::<Vec<_>>();

        assert_eq!(descriptors.len(), 3);
        assert!(
            descriptors
                .iter()
                .all(|descriptor| descriptor.status == CandidateStatus::Wired)
        );
    }

    #[test]
    fn rawpsd_candidate_is_wired() {
        let adapter = candidate_adapters()
            .into_iter()
            .find(|adapter| adapter.descriptor().id == "rust-native")
            .expect("rust-native candidate must be registered");
        let descriptor = adapter.descriptor();

        assert_eq!(descriptor.status, CandidateStatus::Wired);
        assert_eq!(descriptor.runtime, CandidateRuntime::RustNative);
        assert!(descriptor.display_name.contains("rawpsd"));
        assert!(descriptor.notes.contains(RAWPSD_CANDIDATE_VERSION));
    }

    #[test]
    fn rawpsd_candidate_runs_committed_corpus_without_harness_errors() {
        let adapter = RawPsdCandidateAdapter;
        let report = run_candidate(&committed_corpus_path(), &adapter)
            .expect("rawpsd candidate should produce a report");

        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("report should serialize")
        );
        assert_eq!(report.summary.passed, 4);
        assert_eq!(report.summary.failed, 3);
        assert_eq!(report.summary.skipped, 0);
        assert_eq!(report.summary.errors, 0);

        let mut failed = report
            .fixtures
            .iter()
            .filter(|fixture| fixture.status == FixtureStatus::Failed)
            .map(|fixture| fixture.fixture_id.as_str())
            .collect::<Vec<_>>();
        failed.sort_unstable();
        assert_eq!(
            failed,
            vec!["layer-masks", "simple-pixel-layers-psb", "text-layer",]
        );
    }

    #[test]
    fn psd_tools_candidate_is_wired_but_optional() {
        let adapter = candidate_adapters()
            .into_iter()
            .find(|adapter| adapter.descriptor().id == "psd-tools")
            .expect("psd-tools candidate must be registered");
        let descriptor = adapter.descriptor();

        assert_eq!(descriptor.status, CandidateStatus::Wired);
        assert_eq!(descriptor.runtime, CandidateRuntime::Python);
        assert!(descriptor.notes.contains(PSD_TOOLS_REFERENCE_VERSION));
        assert!(descriptor.notes.contains(PSD_TOOLS_REFERENCE_PYTHON));
    }

    #[test]
    fn psd_tools_missing_python_is_reported_as_unavailable() {
        let corpus = load_corpus(&committed_corpus_path()).expect("committed corpus should load");
        let fixture = corpus
            .fixtures
            .first()
            .expect("committed corpus must contain a fixture");
        let root = committed_corpus_path()
            .parent()
            .expect("corpus path must have a parent")
            .to_path_buf();
        let adapter =
            PsdToolsReferenceAdapter::with_python("yu-psd-spike-definitely-missing-python-runtime");

        let error = adapter
            .inspect(&root.join(&fixture.path), fixture)
            .expect_err("missing Python must not be treated as execution success");
        assert_eq!(error.kind, AdapterErrorKind::Unavailable);
    }

    #[test]
    #[ignore = "requires Python 3.12 with psd-tools 1.20.0"]
    fn psd_tools_reference_matches_committed_corpus() {
        let adapter = PsdToolsReferenceAdapter::default();
        let report = run_candidate(&committed_corpus_path(), &adapter)
            .expect("psd-tools reference adapter should produce a report");

        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("report should serialize")
        );
        assert_eq!(report.summary.passed, 7);
        assert_eq!(report.summary.failed, 0);
        assert_eq!(report.summary.skipped, 0);
        assert_eq!(report.summary.errors, 0);
    }

    #[test]
    fn ag_psd_candidate_is_wired_but_optional() {
        let adapter = candidate_adapters()
            .into_iter()
            .find(|adapter| adapter.descriptor().id == "typescript-psd")
            .expect("TypeScript candidate must be registered");
        let descriptor = adapter.descriptor();

        assert_eq!(descriptor.status, CandidateStatus::Wired);
        assert_eq!(descriptor.runtime, CandidateRuntime::TypeScriptNode);
        assert!(descriptor.display_name.contains("ag-psd"));
        assert!(descriptor.notes.contains(AG_PSD_CANDIDATE_VERSION));
        assert!(descriptor.notes.contains(AG_PSD_CANDIDATE_NODE_MAJOR));
    }

    #[test]
    fn ag_psd_missing_node_is_reported_as_unavailable() {
        let corpus = load_corpus(&committed_corpus_path()).expect("committed corpus should load");
        let fixture = corpus
            .fixtures
            .first()
            .expect("committed corpus must contain a fixture");
        let root = committed_corpus_path()
            .parent()
            .expect("corpus path must have a parent")
            .to_path_buf();
        let adapter =
            AgPsdCandidateAdapter::with_node("yu-psd-spike-definitely-missing-node-runtime");

        let error = adapter
            .inspect(&root.join(&fixture.path), fixture)
            .expect_err("missing Node.js must not be treated as execution success");
        assert_eq!(error.kind, AdapterErrorKind::Unavailable);
    }

    #[test]
    #[ignore = "requires Node.js 22 with ag-psd 31.0.2"]
    fn ag_psd_candidate_matches_committed_corpus() {
        let adapter = AgPsdCandidateAdapter::default();
        let report = run_candidate(&committed_corpus_path(), &adapter)
            .expect("ag-psd candidate should produce a report");

        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("report should serialize")
        );
        assert_eq!(report.summary.passed, 7);
        assert_eq!(report.summary.failed, 0);
        assert_eq!(report.summary.skipped, 0);
        assert_eq!(report.summary.errors, 0);
    }

    #[test]
    fn corpus_rejects_parent_directory_traversal() {
        let corpus = PsdCorpus {
            schema_version: CORPUS_SCHEMA_VERSION.to_owned(),
            fixtures: vec![PsdFixture {
                id: "escape".to_owned(),
                path: "../escape.psd".to_owned(),
                format: PsdFormat::Psd,
                features: vec![FixtureFeature::Malformed],
                expected: FixtureExpectation {
                    parse: ParseExpectation::Reject,
                    width: None,
                    height: None,
                    layer_count: None,
                    minimum_tree_depth: None,
                    layer_names: None,
                    text_layer_count: None,
                    pixel_mask_layer_count: None,
                    vector_mask_layer_count: None,
                },
                provenance: FixtureProvenance {
                    kind: FixtureProvenanceKind::Synthetic,
                    source: "test".to_owned(),
                    license: None,
                    redistributable: true,
                    upstream_commit: None,
                    upstream_path: None,
                },
            }],
        };

        let error = validate_corpus(&corpus, Path::new(".")).expect_err("path must be rejected");
        assert!(error.to_string().contains("unsafe path"));
    }
}
