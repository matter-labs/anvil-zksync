use anvil_zksync_common::sh_warn;
use std::collections::HashSet;

use schemars::{
    schema::{Schema, SchemaObject},
    visit::{visit_schema_object, Visitor},
    Map,
};
use serde_json::json;

/// Patch for **known** zkSync-specific fields that are not a part of Ethereum spec. Be mindful
/// adding new stuff here and ensure this is the desired outcome!
pub struct EthSpecPatch {
    schema_name: String,
    additional_properties: Map<String, Schema>,
    not_required_properties: HashSet<String>,
    disable_unevaluated_props: bool,
}

pub struct EthSpecPatchBuilder {
    schema_name: String,
    additional_properties: Map<String, Schema>,
    not_required_properties: HashSet<String>,
    disable_unevaluated_props: bool,
}

impl EthSpecPatchBuilder {
    fn new(schema_name: String) -> Self {
        EthSpecPatchBuilder {
            schema_name,
            additional_properties: Map::default(),
            not_required_properties: HashSet::default(),
            disable_unevaluated_props: false,
        }
    }

    fn build(self) -> EthSpecPatch {
        EthSpecPatch {
            schema_name: self.schema_name,
            additional_properties: self.additional_properties,
            not_required_properties: self.not_required_properties,
            disable_unevaluated_props: self.disable_unevaluated_props,
        }
    }

    fn additional_property(mut self, property_name: String, property_schema: Schema) -> Self {
        self.additional_properties
            .insert(property_name, property_schema);
        self
    }

    fn property_not_required(mut self, property_name: String) -> Self {
        self.not_required_properties.insert(property_name);
        self
    }

    #[allow(dead_code)]
    fn disable_unevaluated_props(mut self) -> Self {
        self.disable_unevaluated_props = true;
        self
    }
}

impl EthSpecPatch {
    pub fn for_block() -> Self {
        // ZKsync introduces two extra L1 batch-related properties for block objects. Along with an
        // empty `sealFields` property (TODO: remove from core and anvil-zksync).
        EthSpecPatchBuilder::new("Block object".to_string())
            .additional_property(
                "l1BatchNumber".to_string(),
                // Null for blocks that are not a part of L1 batch yet
                serde_json::from_value(json!({"oneOf": [{"type": "null"}, {"type": "string", "pattern": "^0x([1-9a-f]+[0-9a-f]*|0)$"}]})).unwrap()
            )
            .additional_property(
                "l1BatchTimestamp".to_string(),
                // Null for blocks that are not a part of L1 batch yet
                serde_json::from_value(json!({"oneOf": [{"type": "null"}, {"type": "string", "pattern": "^0x([1-9a-f]+[0-9a-f]*|0)$"}]})).unwrap()
            )
            .additional_property(
                "sealFields".to_string(),
                // Always empty (both core and anvil-zksync)
                serde_json::from_value(json!({"const": []})).unwrap(),
            )
            .build()
    }

    pub fn for_tx_info() -> Self {
        // ZKsync introduces two extra L1 batch-related properties for all transaction types.
        EthSpecPatchBuilder::new("Transaction information".to_string())
            .additional_property(
                "l1BatchNumber".to_string(),
                // Null for txs that are not a part of L1 batch yet
                serde_json::from_value(json!({"oneOf": [{"type": "null"}, {"type": "string", "pattern": "^0x([1-9a-f]+[0-9a-f]*|0)$"}]})).unwrap()
            )
            .additional_property(
                "l1BatchTxIndex".to_string(),
                // Null for txs that are not a part of L1 batch yet
                serde_json::from_value(json!({"oneOf": [{"type": "null"}, {"type": "string", "pattern": "^0x([1-9a-f]+[0-9a-f]*|0)$"}]})).unwrap()
            )
            .build()
    }

    pub fn for_legacy_tx() -> Self {
        // ZKsync assumes that legacy transactions can have `type` unset which is the industry
        // standard across ETH tooling. Presumably for better backwards compatibility.
        //
        // We also include `maxFeePerGas` and `maxPriorityFeePerGas` which are only relevant for
        // EIP1559 transactions. TODO: Figure out if we want to avoid this behavior
        EthSpecPatchBuilder::new("Signed Legacy Transaction".to_string())
            .additional_property(
                "maxFeePerGas".to_string(),
                serde_json::from_value(json!({"pattern": "^0x([1-9a-f]+[0-9a-f]*|0)$"})).unwrap(),
            )
            .additional_property(
                "maxPriorityFeePerGas".to_string(),
                serde_json::from_value(json!({"pattern": "^0x([1-9a-f]+[0-9a-f]*|0)$"})).unwrap(),
            )
            .property_not_required("type".to_string())
            .build()
    }
}

// JSON Schema visitor implementation that applies specific patch to the supplied schema.
impl Visitor for EthSpecPatch {
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
                panic!(
                    "Failed to interpret schema `{}` as an object",
                    self.schema_name,
                );
            };
            // Add additional properties to the `properties` field
            object_validation
                .properties
                .append(&mut self.additional_properties.clone());
            // Removed not required properties from the `required` field
            object_validation
                .required
                .retain(|f| !self.not_required_properties.contains(f));
            if self.disable_unevaluated_props {
                // Remove `unevaluatedProperties` and expect it to be present and set to `false`.
                let removed = schema.extensions.remove("unevaluatedProperties");
                if removed != Some(json!(false)) {
                    sh_warn!("Removed 'unevaluatedProperties' from schema but it wasn't set to `false`: {}",
                        self.schema_name,
                    );
                }
            }
        };
        apply_patch();
        visit_schema_object(self, schema)
    }
}
