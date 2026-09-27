use crate::{PsdReadResult, ReadOperation};
use yu_capability_psd::{
    PSD_CONTRACT_VERSION, PsdDocumentInfo, PsdLayerKind, PsdLayerNode, PsdLayerSummary,
};

pub(crate) fn validate(operation: &ReadOperation, result: &PsdReadResult) -> Result<(), String> {
    let version = match result {
        PsdReadResult::Inspect(result) => &result.contract_version,
        PsdReadResult::Tree(result) => &result.contract_version,
        PsdReadResult::LayerList(result) => &result.contract_version,
        PsdReadResult::LayerInfo(result) => &result.contract_version,
    };
    if version != PSD_CONTRACT_VERSION {
        return Err(format!("unsupported PSD contract version: {version}"));
    }
    let document = result.document();
    if document.width == 0 || document.height == 0 {
        return Err("document dimensions must be positive".into());
    }
    if (document.layer_count == 0) != (document.maximum_tree_depth == 0)
        || document.maximum_tree_depth > document.layer_count
    {
        return Err("document layer count/depth disagree".into());
    }
    match result {
        PsdReadResult::Inspect(_) => Ok(()),
        PsdReadResult::LayerList(result) => {
            validate_flat(&result.layers.iter().collect::<Vec<_>>(), document)
        }
        PsdReadResult::Tree(result) => {
            let mut flat = Vec::new();
            let mut stack: Vec<(&PsdLayerNode, Option<&str>, usize)> = result
                .layers
                .iter()
                .rev()
                .map(|node| (node, None, 1))
                .collect();
            while let Some((node, parent, depth)) = stack.pop() {
                if node.layer.parent_id.as_ref().map(|id| id.as_str()) != parent
                    || node.layer.depth != depth
                    || node.layer.child_count != node.children.len()
                {
                    return Err(
                        "tree node parent/depth/child count disagrees with tree structure".into(),
                    );
                }
                flat.push(&node.layer);
                stack.extend(
                    node.children
                        .iter()
                        .rev()
                        .map(|child| (child, Some(node.layer.id.as_str()), depth + 1)),
                );
            }
            validate_flat(&flat, document)
        }
        PsdReadResult::LayerInfo(result) => {
            if let ReadOperation::LayerInfo(requested_id) = operation
                && &result.layer.id != requested_id
            {
                return Err("layer info ID does not match requested ID".into());
            }
            validate_summary(&result.layer, document)
        }
    }
}

fn validate_summary(layer: &PsdLayerSummary, document: &PsdDocumentInfo) -> Result<(), String> {
    let index = layer.id.index();
    if index > document.layer_count || layer.depth == 0 || layer.depth > document.maximum_tree_depth
    {
        return Err("layer ID/depth exceeds document bounds".into());
    }
    if (layer.depth == 1) != layer.parent_id.is_none()
        || layer
            .parent_id
            .as_ref()
            .is_some_and(|id| id.index() >= index)
    {
        return Err("layer parent ID is inconsistent with preorder".into());
    }
    if layer.child_count > document.layer_count - index
        || (layer.child_count > 0 && layer.kind != PsdLayerKind::Group)
    {
        return Err("layer child count/kind is inconsistent".into());
    }
    Ok(())
}

fn validate_flat(layers: &[&PsdLayerSummary], document: &PsdDocumentInfo) -> Result<(), String> {
    if layers.len() != document.layer_count {
        return Err("layer count does not match document".into());
    }
    let mut ancestors: Vec<&PsdLayerSummary> = Vec::new();
    let mut child_counts = vec![0usize; layers.len()];
    let mut maximum_depth = 0;
    for (offset, layer) in layers.iter().enumerate() {
        validate_summary(layer, document)?;
        if layer.id.index() != offset + 1 || layer.depth > ancestors.len() + 1 {
            return Err("layer IDs/depths are not canonical preorder".into());
        }
        ancestors.truncate(layer.depth - 1);
        let parent = ancestors.last();
        if layer.parent_id.as_ref() != parent.map(|item| &item.id) {
            return Err("layer parent does not match preorder ancestry".into());
        }
        if let Some(parent) = parent {
            child_counts[parent.id.index() - 1] += 1;
        }
        maximum_depth = maximum_depth.max(layer.depth);
        ancestors.push(layer);
    }
    if maximum_depth != document.maximum_tree_depth {
        return Err("maximum tree depth does not match document".into());
    }
    if layers
        .iter()
        .zip(child_counts)
        .any(|(layer, count)| layer.child_count != count)
    {
        return Err("declared child counts disagree with preorder".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use yu_capability_psd::PsdLayerListResult;

    fn fixture() -> PsdReadResult {
        PsdReadResult::LayerList(serde_json::from_value::<PsdLayerListResult>(json!({
            "contract_version":"1", "document":{"format":"psd","width":1,"height":1,
            "channels":3,"bits_per_channel":8,"color_mode":"rgb","layer_count":2,"maximum_tree_depth":2},
            "layers":[
                {"id":"L0001","depth":1,"name":"same","kind":"group","visible":true,
                 "has_pixel_mask":false,"has_vector_mask":false,"child_count":1},
                {"id":"L0002","parent_id":"L0001","depth":2,"name":"same","kind":"pixel","visible":true,
                 "has_pixel_mask":false,"has_vector_mask":false,"child_count":0}
            ]
        })).unwrap())
    }
    #[test]
    fn duplicate_names_with_distinct_preorder_ids_are_valid() {
        validate(&ReadOperation::LayerList, &fixture()).unwrap();
    }
    #[test]
    fn invalid_parent_or_id_or_count_or_contract_is_rejected() {
        for mode in 0..5 {
            let mut value = fixture();
            if let PsdReadResult::LayerList(result) = &mut value {
                match mode {
                    0 => result.layers[1].parent_id = None,
                    1 => result.layers[1].id = result.layers[0].id.clone(),
                    2 => result.layers[0].child_count = 0,
                    3 => result.contract_version = "2".into(),
                    _ => result.document.layer_count = usize::MAX,
                }
            }
            assert!(validate(&ReadOperation::LayerList, &value).is_err());
        }
    }
}
