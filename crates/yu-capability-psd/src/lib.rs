use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{error::Error, fmt, str::FromStr};
use yu_engine_api::ExternalEngineRequest;

pub const PSD_CONTRACT_VERSION: &str = "1";

pub const PSD_INSPECT: &str = "psd.inspect";
pub const PSD_TREE: &str = "psd.tree";
pub const PSD_LAYER_LIST: &str = "psd.layer.list";
pub const PSD_LAYER_INFO: &str = "psd.layer.info";
pub const PSD_LAYER_EXPORT: &str = "psd.layer.export";

pub const PSD_V1_CAPABILITIES: [&str; 5] = [
    PSD_INSPECT,
    PSD_TREE,
    PSD_LAYER_LIST,
    PSD_LAYER_INFO,
    PSD_LAYER_EXPORT,
];

pub fn is_v1_capability(value: &str) -> bool {
    PSD_V1_CAPABILITIES.contains(&value)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PsdLayerId(String);

impl PsdLayerId {
    pub fn from_index(index: usize) -> Result<Self, PsdContractError> {
        if index == 0 {
            return Err(PsdContractError::new(
                "PSD layer IDs are one-based; index zero is invalid",
            ));
        }
        Ok(Self(format!("L{index:04}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn index(&self) -> usize {
        self.0[1..]
            .parse()
            .expect("validated PSD layer ID should always contain digits")
    }
}

impl fmt::Display for PsdLayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for PsdLayerId {
    type Err = PsdContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() < 5
            || !value.starts_with('L')
            || !value[1..].bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(PsdContractError::new(format!(
                "invalid PSD layer ID: {value}"
            )));
        }

        let index = value[1..]
            .parse::<usize>()
            .map_err(|_| PsdContractError::new(format!("invalid PSD layer ID: {value}")))?;
        let canonical = Self::from_index(index)?;
        if canonical.as_str() != value {
            return Err(PsdContractError::new(format!(
                "non-canonical PSD layer ID: {value}"
            )));
        }
        Ok(canonical)
    }
}

impl Serialize for PsdLayerId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for PsdLayerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsdFormat {
    Psd,
    Psb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsdColorMode {
    Bitmap,
    Grayscale,
    Indexed,
    Rgb,
    Cmyk,
    Multichannel,
    Duotone,
    Lab,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsdLayerKind {
    Group,
    Pixel,
    Text,
    Shape,
    SmartObject,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdBounds {
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdDocumentInfo {
    pub format: PsdFormat,
    pub width: u32,
    pub height: u32,
    pub channels: u16,
    pub bits_per_channel: u16,
    pub color_mode: PsdColorMode,
    pub layer_count: usize,
    pub maximum_tree_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdLayerSummary {
    pub id: PsdLayerId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<PsdLayerId>,
    pub depth: usize,
    pub name: String,
    pub kind: PsdLayerKind,
    pub visible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<PsdBounds>,
    pub has_pixel_mask: bool,
    pub has_vector_mask: bool,
    pub child_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdLayerNode {
    #[serde(flatten)]
    pub layer: PsdLayerSummary,
    pub children: Vec<PsdLayerNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdInspectRequest {
    pub input_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdInspectResult {
    pub contract_version: String,
    pub document: PsdDocumentInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdTreeRequest {
    pub input_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdTreeResult {
    pub contract_version: String,
    pub document: PsdDocumentInfo,
    pub layers: Vec<PsdLayerNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdLayerListRequest {
    pub input_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdLayerListResult {
    pub contract_version: String,
    pub document: PsdDocumentInfo,
    pub layers: Vec<PsdLayerSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdLayerInfoRequest {
    pub input_path: String,
    pub layer_id: PsdLayerId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdLayerInfoResult {
    pub contract_version: String,
    pub document: PsdDocumentInfo,
    pub layer: PsdLayerSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdLayerExportRequest {
    pub input_path: String,
    pub layer_id: PsdLayerId,
    pub output_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsdPixelFormat {
    Rgba8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsdExportContainer {
    Png,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsdLayerExportResult {
    pub contract_version: String,
    pub layer_id: PsdLayerId,
    pub output_path: String,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PsdPixelFormat,
    pub container: PsdExportContainer,
}

pub fn inspect_engine_request(
    request_id: impl Into<String>,
    input_path: impl Into<String>,
) -> ExternalEngineRequest<PsdInspectRequest> {
    ExternalEngineRequest::new(
        request_id,
        PSD_INSPECT,
        PsdInspectRequest {
            input_path: input_path.into(),
        },
    )
}

pub fn tree_engine_request(
    request_id: impl Into<String>,
    input_path: impl Into<String>,
) -> ExternalEngineRequest<PsdTreeRequest> {
    ExternalEngineRequest::new(
        request_id,
        PSD_TREE,
        PsdTreeRequest {
            input_path: input_path.into(),
        },
    )
}

pub fn layer_list_engine_request(
    request_id: impl Into<String>,
    input_path: impl Into<String>,
) -> ExternalEngineRequest<PsdLayerListRequest> {
    ExternalEngineRequest::new(
        request_id,
        PSD_LAYER_LIST,
        PsdLayerListRequest {
            input_path: input_path.into(),
        },
    )
}

pub fn layer_info_engine_request(
    request_id: impl Into<String>,
    input_path: impl Into<String>,
    layer_id: PsdLayerId,
) -> ExternalEngineRequest<PsdLayerInfoRequest> {
    ExternalEngineRequest::new(
        request_id,
        PSD_LAYER_INFO,
        PsdLayerInfoRequest {
            input_path: input_path.into(),
            layer_id,
        },
    )
}

pub fn layer_export_engine_request(
    request_id: impl Into<String>,
    input_path: impl Into<String>,
    layer_id: PsdLayerId,
    output_path: impl Into<String>,
) -> ExternalEngineRequest<PsdLayerExportRequest> {
    ExternalEngineRequest::new(
        request_id,
        PSD_LAYER_EXPORT,
        PsdLayerExportRequest {
            input_path: input_path.into(),
            layer_id,
            output_path: output_path.into(),
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsdLayerDraft {
    pub name: String,
    pub kind: PsdLayerKind,
    pub visible: bool,
    pub bounds: Option<PsdBounds>,
    pub has_pixel_mask: bool,
    pub has_vector_mask: bool,
    pub children: Vec<PsdLayerDraft>,
}

pub fn assign_stable_layer_ids(
    layers: Vec<PsdLayerDraft>,
) -> Result<Vec<PsdLayerNode>, PsdContractError> {
    let mut next_index = 1usize;
    assign_layer_ids_at_depth(layers, None, 1, &mut next_index)
}

fn assign_layer_ids_at_depth(
    layers: Vec<PsdLayerDraft>,
    parent_id: Option<PsdLayerId>,
    depth: usize,
    next_index: &mut usize,
) -> Result<Vec<PsdLayerNode>, PsdContractError> {
    let mut result = Vec::with_capacity(layers.len());

    for draft in layers {
        let id = PsdLayerId::from_index(*next_index)?;
        *next_index = next_index
            .checked_add(1)
            .ok_or_else(|| PsdContractError::new("PSD layer ID space exhausted"))?;

        let child_count = draft.children.len();
        let children = assign_layer_ids_at_depth(
            draft.children,
            Some(id.clone()),
            depth + 1,
            next_index,
        )?;

        result.push(PsdLayerNode {
            layer: PsdLayerSummary {
                id,
                parent_id: parent_id.clone(),
                depth,
                name: draft.name,
                kind: draft.kind,
                visible: draft.visible,
                bounds: draft.bounds,
                has_pixel_mask: draft.has_pixel_mask,
                has_vector_mask: draft.has_vector_mask,
                child_count,
            },
            children,
        });
    }

    Ok(result)
}

pub fn flatten_layer_tree(layers: &[PsdLayerNode]) -> Vec<PsdLayerSummary> {
    let mut result = Vec::new();
    flatten_layer_nodes(layers, &mut result);
    result
}

fn flatten_layer_nodes(layers: &[PsdLayerNode], result: &mut Vec<PsdLayerSummary>) {
    for node in layers {
        result.push(node.layer.clone());
        flatten_layer_nodes(&node.children, result);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsdContractError {
    pub message: String,
}

impl PsdContractError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PsdContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for PsdContractError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(name: &str, children: Vec<PsdLayerDraft>) -> PsdLayerDraft {
        PsdLayerDraft {
            name: name.to_owned(),
            kind: if children.is_empty() {
                PsdLayerKind::Pixel
            } else {
                PsdLayerKind::Group
            },
            visible: true,
            bounds: None,
            has_pixel_mask: false,
            has_vector_mask: false,
            children,
        }
    }

    #[test]
    fn layer_ids_are_canonical_and_round_trip_json() {
        for (index, expected) in [(1, "L0001"), (42, "L0042"), (9999, "L9999"), (10000, "L10000")] {
            let id = PsdLayerId::from_index(index).unwrap();
            assert_eq!(id.as_str(), expected);
            assert_eq!(id.index(), index);
            assert_eq!(expected.parse::<PsdLayerId>().unwrap(), id);

            let json = serde_json::to_string(&id).unwrap();
            assert_eq!(serde_json::from_str::<PsdLayerId>(&json).unwrap(), id);
        }

        for invalid in ["", "L0000", "L001", "L00001", "l0001", "L00A1"] {
            assert!(invalid.parse::<PsdLayerId>().is_err(), "{invalid}");
        }
    }

    #[test]
    fn stable_ids_follow_logical_preorder_depth_first_traversal() {
        let tree = assign_stable_layer_ids(vec![
            draft(
                "Group",
                vec![draft("Child A", vec![]), draft("Child B", vec![])],
            ),
            draft("Root Pixel", vec![]),
        ])
        .unwrap();

        let flat = flatten_layer_tree(&tree);
        let ids = flat
            .iter()
            .map(|layer| layer.id.as_str())
            .collect::<Vec<_>>();
        let names = flat
            .iter()
            .map(|layer| layer.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["L0001", "L0002", "L0003", "L0004"]);
        assert_eq!(names, vec!["Group", "Child A", "Child B", "Root Pixel"]);
        assert_eq!(flat[0].parent_id, None);
        assert_eq!(flat[1].parent_id.as_ref().unwrap().as_str(), "L0001");
        assert_eq!(flat[2].parent_id.as_ref().unwrap().as_str(), "L0001");
        assert_eq!(flat[3].parent_id, None);
        assert_eq!(flat[0].depth, 1);
        assert_eq!(flat[1].depth, 2);
    }

    #[test]
    fn duplicate_names_do_not_affect_layer_identity() {
        let tree = assign_stable_layer_ids(vec![
            draft("X", vec![]),
            draft("X", vec![]),
            draft("X", vec![]),
        ])
        .unwrap();
        let flat = flatten_layer_tree(&tree);

        assert_eq!(flat[0].name, flat[1].name);
        assert_ne!(flat[0].id, flat[1].id);
        assert_ne!(flat[1].id, flat[2].id);
    }

    #[test]
    fn request_builders_use_frozen_capability_ids() {
        let layer_id = PsdLayerId::from_index(7).unwrap();

        assert_eq!(
            inspect_engine_request("req-1", "/tmp/a.psd").capability,
            PSD_INSPECT
        );
        assert_eq!(tree_engine_request("req-2", "/tmp/a.psd").capability, PSD_TREE);
        assert_eq!(
            layer_list_engine_request("req-3", "/tmp/a.psd").capability,
            PSD_LAYER_LIST
        );
        assert_eq!(
            layer_info_engine_request("req-4", "/tmp/a.psd", layer_id.clone()).capability,
            PSD_LAYER_INFO
        );
        assert_eq!(
            layer_export_engine_request("req-5", "/tmp/a.psd", layer_id, "/tmp/a.png").capability,
            PSD_LAYER_EXPORT
        );
    }

    #[test]
    fn committed_contract_snapshot_guards_v1_semantics() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/data/psd-capability-contract-v1.json");
        let content = std::fs::read_to_string(path).expect("PSD contract snapshot should exist");
        let snapshot: serde_json::Value =
            serde_json::from_str(&content).expect("PSD contract snapshot should parse");

        assert_eq!(snapshot["schema_version"], "1");
        assert_eq!(snapshot["psd_contract_version"], PSD_CONTRACT_VERSION);
        assert_eq!(snapshot["external_engine_protocol_version"], "1");
        assert_eq!(
            snapshot["layer_id"]["traversal"],
            "logical_layer_tree_preorder_depth_first"
        );
        assert_eq!(snapshot["layer_id"]["identity"], "independent_of_layer_name");
        assert_eq!(
            snapshot["capabilities"]["psd.layer.export"]["v0_1_scope"]["normalized_pixel_format"],
            "rgba8"
        );
        assert_eq!(
            snapshot["capabilities"]["psd.layer.export"]["v0_1_scope"]["high_bit_depth"],
            "unsupported"
        );
        assert_eq!(snapshot["protocol"]["transport"], "one_process_one_request");
        assert_eq!(snapshot["protocol"]["business_error_exit_code"], 0);
        assert_eq!(snapshot["protocol"]["malformed_transport_exit_code"], 2);
        assert_eq!(snapshot["protocol"]["request_id_must_round_trip"], true);
        assert_eq!(
            snapshot["reference_adapter"]["implemented_capabilities"]
                .as_array()
                .expect("implemented capabilities should be an array")
                .len(),
            4
        );
        assert_eq!(
            snapshot["reference_adapter"]["deferred_capabilities"][0],
            PSD_LAYER_EXPORT
        );
    }

    #[test]
    fn tree_shape_serializes_without_backend_specific_fields() {
        let tree = assign_stable_layer_ids(vec![draft("Background", vec![])]).unwrap();
        let result = PsdTreeResult {
            contract_version: PSD_CONTRACT_VERSION.to_owned(),
            document: PsdDocumentInfo {
                format: PsdFormat::Psd,
                width: 100,
                height: 50,
                channels: 4,
                bits_per_channel: 8,
                color_mode: PsdColorMode::Rgb,
                layer_count: 1,
                maximum_tree_depth: 1,
            },
            layers: tree,
        };

        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["contract_version"], "1");
        assert_eq!(json["document"]["format"], "psd");
        assert_eq!(json["document"]["color_mode"], "rgb");
        assert_eq!(json["layers"][0]["id"], "L0001");
        assert_eq!(json["layers"][0]["name"], "Background");
        assert!(json["layers"][0].get("backend_id").is_none());
    }
}
