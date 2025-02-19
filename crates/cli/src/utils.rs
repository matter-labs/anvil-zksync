use crate::cli::{Command, ForkUrl};
use anvil_zksync_config::types::Genesis;
use anvil_zksync_config::TestNodeConfig;
use anvil_zksync_core::node::fork::ForkDetails;
use std::fs;
use zksync_telemetry::TelemetryProps;

/// Parses the genesis file from the given path.
pub fn parse_genesis_file(path: &str) -> Result<Genesis, String> {
    let file_content =
        fs::read_to_string(path).map_err(|err| format!("Failed to read file: {err}"))?;
    serde_json::from_str(&file_content).map_err(|err| format!("Failed to parse JSON: {err}"))
}

/// Updates the configuration from fork details.
pub async fn update_with_fork_details(config: &mut TestNodeConfig, fd: &ForkDetails) {
    let l1_gas_price = config.l1_gas_price.or(Some(fd.l1_gas_price));
    let l2_gas_price = config.l2_gas_price.or(Some(fd.l2_fair_gas_price));
    let l1_pubdata_price = config.l1_pubdata_price.or(Some(fd.fair_pubdata_price));
    let price_scale = config
        .price_scale_factor
        .or(Some(fd.estimate_gas_price_scale_factor));
    let gas_limit_scale = config
        .limit_scale_factor
        .or(Some(fd.estimate_gas_scale_factor));
    let chain_id = config.chain_id.or(Some(fd.chain_id.as_u64() as u32));

    config
        .update_l1_gas_price(l1_gas_price)
        .update_l2_gas_price(l2_gas_price)
        .update_l1_pubdata_price(l1_pubdata_price)
        .update_price_scale(price_scale)
        .update_gas_limit_scale(gas_limit_scale)
        .update_chain_id(chain_id);
}

pub const TELEMETRY_SENSITIVE_VALUE: &str = "***";

pub fn get_cli_command_telemetry_props(command: Option<Command>) -> Option<TelemetryProps> {
    let get_sensitive_fork_url = |fork_url| match fork_url {
        ForkUrl::Other(_) => Some(TELEMETRY_SENSITIVE_VALUE.to_string()),
        _ => Some(format!("{:?}", fork_url)),
    };

    let (command_name, command_args) = match command {
        Some(Command::Run) => (Some("run"), None),
        Some(Command::Fork(args)) => {
            let command_args = TelemetryProps::new()
                .insert_with("fork_url", args.fork_url, get_sensitive_fork_url)
                .insert(
                    "fork_block_number",
                    args.fork_block_number.map(serde_json::Number::from),
                )
                .insert(
                    "fork_transaction_hash",
                    args.fork_transaction_hash.map(|h| h.to_string()),
                )
                .take();
            (Some("fork"), Some(command_args))
        }
        Some(Command::ReplayTx(args)) => {
            let command_args = TelemetryProps::new()
                .insert_with("fork_url", args.fork_url, get_sensitive_fork_url)
                .insert_with("tx", args.tx, |_| Some(TELEMETRY_SENSITIVE_VALUE))
                .take();
            (Some("replay_tx"), Some(command_args))
        }
        None => (None, None),
    };

    command_name?;
    Some(
        TelemetryProps::new()
            .insert("name", command_name)
            .insert("args", command_args)
            .take(),
    )
}
