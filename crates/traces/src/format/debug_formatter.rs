// NEW imports
use alloy::primitives::{Address as AAddress, Bytes as ABytes, U256 as AU256};
use serde::Deserialize;
use zksync_multivm::interface::{Call, CallType};
use zksync_types::zk_evm_types::FarCallOpcode;
use zksync_types::{H160, U256 as ZkU256};

#[derive(Debug, Deserialize)]
pub struct DebugTraceEnvelope {
    pub result: DebugCallNode,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DebugCallNode {
    #[serde(default)]
    pub calls: Vec<DebugCallNode>,

    pub from: AAddress,
    pub to: AAddress,

    #[serde(default)]
    pub gas: AU256,
    #[serde(default, rename = "gasUsed")]
    pub gas_used: AU256,
    #[serde(default)]
    pub value: AU256,

    #[serde(rename = "type")]
    pub call_type: String,

    #[serde(default)]
    pub input: ABytes,
    #[serde(default)]
    pub output: ABytes,

    #[serde(default)]
    pub error: Option<String>,
    #[serde(default, rename = "revertReason")]
    pub revert_reason: Option<String>,
}

#[inline]
fn addr_alloy_to_zk(a: &AAddress) -> H160 {
    H160::from_slice(a.as_slice())
}

#[inline]
fn u256_alloy_to_zk(a: AU256) -> ZkU256 {
    let be = a.to_be_bytes::<32>();
    ZkU256::from_big_endian(&be)
}

#[inline]
fn u256_to_u64_sat(a: AU256) -> u64 {
    let limbs = a.as_limbs();
    if limbs[1] != 0 || limbs[2] != 0 || limbs[3] != 0 {
        u64::MAX
    } else {
        limbs[0]
    }
}

fn node_to_call(n: &DebugCallNode, parent_gas: u64) -> Call {
    let subcalls: Vec<Call> = n
        .calls
        .iter()
        .map(|c| node_to_call(c, u256_to_u64_sat(n.gas)))
        .collect();

    let call_type = match n.call_type.to_ascii_lowercase().as_str() {
        "call" => CallType::Call(FarCallOpcode::Normal),
        "delegatecall" => CallType::Call(FarCallOpcode::Delegate),
        "staticcall" | "mimiccall" => CallType::Call(FarCallOpcode::Mimic),
        "create" | "create2" => CallType::Create,
        _ => CallType::Call(FarCallOpcode::Normal),
    };

    Call {
        r#type: call_type,
        from: addr_alloy_to_zk(&n.from),
        to: addr_alloy_to_zk(&n.to),
        parent_gas,
        gas: u256_to_u64_sat(n.gas),
        gas_used: u256_to_u64_sat(n.gas_used),
        value: u256_alloy_to_zk(n.value),
        input: n.input.to_vec(),
        output: n.output.to_vec(),
        error: n.error.clone(),
        revert_reason: n.revert_reason.clone(),
        calls: subcalls,
    }
}

pub fn calls_from_debug_json(json: &str) -> anyhow::Result<Vec<Call>> {
    let env: DebugTraceEnvelope = serde_json::from_str(json)?;
    let root_gas_u64 = u256_to_u64_sat(env.result.gas);

    Ok(env
        .result
        .calls
        .iter()
        .map(|c| node_to_call(c, root_gas_u64))
        .collect())
}
