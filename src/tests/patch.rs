use schemars::schema::{Schema, SchemaObject};
use schemars::visit::{visit_schema_object, Visitor};
use schemars::Map;
use serde_json::json;

/// Patch for **known** zkSync-specific fields that are not a part of Ethereum spec. Be mindful
/// adding new stuff here and ensure this is the desired outcome!
pub struct Patch {
    schema_name: String,
    additional_properties: Map<String, Schema>,
}

impl Patch {
    pub fn for_block() -> Self {
        Self {
            schema_name: "Block object".to_string(),
            additional_properties: [
                (
                    "l1BatchNumber".to_string(),
                    // Null for blocks that are not a part of L1 batch yet
                    serde_json::from_value(json!({"oneOf": [{"type": "null"}, {"type": "string", "pattern": "^0x([1-9a-f]+[0-9a-f]*|0)$"}]})).unwrap(),
                ),
                (
                    "l1BatchTimestamp".to_string(),
                    // Null for blocks that are not a part of L1 batch yet
                    serde_json::from_value(json!({"oneOf": [{"type": "null"}, {"type": "string", "pattern": "^0x([1-9a-f]+[0-9a-f]*|0)$"}]})).unwrap(),
                ),
                (
                    "sealFields".to_string(),
                    // Always empty (both core and era-test-node)
                    serde_json::from_value(json!({"const": []})).unwrap(),
                ),
            ].into(),
        }
    }
}

impl Visitor for Patch {
    fn visit_schema_object(&mut self, schema: &mut SchemaObject) {
        // We need to always call `visit_schema_object` at the end of this function's flow.
        // Below is a little trick to still be able do early return without copy-pasting the
        // `visit_schema_object` invocation.
        let mut apply_patch = || {
            let Some(metadata) = &schema.metadata else {
                return;
            };
            if !metadata
                .title
                .as_ref()
                .map(|t| t == &self.schema_name)
                .unwrap_or_default()
            {
                return;
            }
            let Some(object_validation) = &mut schema.object else {
                return;
            };
            object_validation
                .properties
                .append(&mut self.additional_properties.clone());
        };
        apply_patch();
        visit_schema_object(self, schema)
    }
}
