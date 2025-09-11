use anvil_zksync_types::serde_helpers::{bytes_hex, h160_hex, u64_hex, u256_hex};
use serde::Deserialize;
use zksync_multivm::interface::{Call, CallType};
use zksync_types::zk_evm_types::FarCallOpcode;
use zksync_types::{H160, U256};

#[derive(Debug, Deserialize)]
pub struct DebugTraceEnvelope {
    pub result: DebugCallNode,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DebugCallNode {
    #[serde(default)]
    pub calls: Vec<DebugCallNode>,

    #[serde(with = "h160_hex")]
    pub from: H160,
    #[serde(with = "h160_hex")]
    pub to: H160,

    #[serde(default, with = "u64_hex")]
    pub gas: u64,
    #[serde(default, rename = "gasUsed", with = "u64_hex")]
    pub gas_used: u64,
    #[serde(default, with = "u256_hex")]
    pub value: U256,

    #[serde(rename = "type")]
    pub call_type: String,

    #[serde(default, with = "bytes_hex")]
    pub input: Vec<u8>,
    #[serde(default, with = "bytes_hex")]
    pub output: Vec<u8>,

    #[serde(default)]
    pub error: Option<String>,
    #[serde(default, rename = "revertReason")]
    pub revert_reason: Option<String>,
}

fn node_to_call(n: &DebugCallNode, parent_gas: u64) -> Call {
    let subcalls = n.calls.iter().map(|c| node_to_call(c, n.gas)).collect();

    let call_type = match n.call_type.to_ascii_lowercase().as_str() {
        "call" => CallType::Call(FarCallOpcode::Normal),
        "delegatecall" => CallType::Call(FarCallOpcode::Delegate),
        "staticcall" | "mimiccall" => CallType::Call(FarCallOpcode::Mimic),
        "create" | "create2" => CallType::Create,
        _ => CallType::Call(FarCallOpcode::Normal),
    };

    Call {
        r#type: call_type,
        from: n.from,
        to: n.to,
        parent_gas,
        gas: n.gas,
        gas_used: n.gas_used,
        value: n.value,
        input: n.input.clone(),
        output: n.output.clone(),
        error: n.error.clone(),
        revert_reason: n.revert_reason.clone(),
        calls: subcalls,
    }
}

pub fn calls_from_debug_json(json: &str) -> anyhow::Result<Vec<Call>> {
    let env: DebugTraceEnvelope = serde_json::from_str(json)?;
    let root_gas = env.result.gas;
    Ok(env
        .result
        .calls
        .iter()
        .map(|c| node_to_call(c, root_gas))
        .collect())
}
