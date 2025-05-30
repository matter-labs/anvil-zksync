use std::fmt::{Display, Write};

use anvil_zksync_common::utils::cost::format_gwei;
use colored::Colorize as _;
use hex::ToHex;
use zksync_types::{Address, ExecuteTransactionCommon, Transaction};

use crate::formatter::PrettyFmt;

#[derive(Debug)]
pub struct PrettyTransaction<'a>(pub &'a Transaction);
#[derive(Debug)]
pub struct PrettyTransactionEstimationView<'a>(pub &'a Transaction);

impl PrettyFmt for PrettyTransaction<'_> {
    fn pretty_fmt(&self, w: &mut impl Write) -> std::fmt::Result {
        let tx = self.0;
        writeln!(w, "    |")?;
        writeln!(w, "    | {}", "Transaction details:".cyan())?;
        writeln!(w, "    |   Transaction Type: {:?}", tx.tx_format())?;
        if let Some(nonce) = tx.nonce() {
            writeln!(w, "    |   Nonce: {nonce}")?;
        }
        if let Some(contract_address) = tx.recipient_account() {
            writeln!(w, "    |   To: {contract_address:?}")?;
        }
        writeln!(w, "    |   From: {:?}", tx.initiator_account())?;
        if let ExecuteTransactionCommon::L2(l2_tx) = &tx.common_data {
            if let Some(input_data) = &l2_tx.input {
                let hex_data: String = input_data.data.encode_hex();
                writeln!(w, "    |   Input Data: 0x{hex_data}")?;
                writeln!(w, "    |   Hash: {:?}", tx.hash())?;
            }
        }
        writeln!(w, "    |   gas limit: {}", tx.gas_limit())?;
        writeln!(
            w,
            "    |   Gas Price: {}",
            format_gwei(tx.max_fee_per_gas())
        )?;
        writeln!(
            w,
            "    |   Gas Per Pubdata Limit: {}",
            tx.gas_per_pubdata_byte_limit()
        )?;

        // Log paymaster details if available.
        if let ExecuteTransactionCommon::L2(l2_tx) = &tx.common_data {
            let paymaster_address = l2_tx.paymaster_params.paymaster;
            let paymaster_input = &l2_tx.paymaster_params.paymaster_input;
            if paymaster_address != Address::zero() || !paymaster_input.is_empty() {
                writeln!(w, "    | {}", "Paymaster details:".cyan())?;
                writeln!(w, "    |   Paymaster Address: {paymaster_address:?}")?;
                let paymaster_input_str = if paymaster_input.is_empty() {
                    "None".to_string()
                } else {
                    paymaster_input.encode_hex()
                };
                writeln!(w, "    |   Paymaster Input: 0x{paymaster_input_str}")?;
            }
        }
        Ok(())
    }
}

impl Display for PrettyTransaction<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}

impl PrettyFmt for PrettyTransactionEstimationView<'_> {
    fn pretty_fmt(&self, w: &mut impl Write) -> std::fmt::Result {
        let tx = self.0;
        writeln!(w, "    |")?;
        writeln!(w, "    | {}", "Transaction details:".cyan())?;
        writeln!(w, "    |   Transaction Type: {:?}", tx.tx_format())?;
        if let Some(nonce) = tx.nonce() {
            writeln!(w, "    |   Nonce: {nonce}")?;
        }
        if let Some(contract_address) = tx.recipient_account() {
            writeln!(w, "    |   To: {contract_address:?}")?;
        }
        writeln!(w, "    |   From: {:?}", tx.initiator_account())?;
        if let ExecuteTransactionCommon::L2(l2_tx) = &tx.common_data {
            if let Some(input_data) = &l2_tx.input {
                let hex_data: String = input_data.data.encode_hex();
                writeln!(w, "    |   Input Data: 0x{hex_data}")?;
                writeln!(w, "    |   Hash: {:?}", tx.hash())?;
            }
        }

        // Log paymaster details if available.
        if let ExecuteTransactionCommon::L2(l2_tx) = &tx.common_data {
            let paymaster_address = l2_tx.paymaster_params.paymaster;
            let paymaster_input = &l2_tx.paymaster_params.paymaster_input;
            if paymaster_address != Address::zero() || !paymaster_input.is_empty() {
                writeln!(w, "    | {}", "Paymaster details:".cyan())?;
                writeln!(w, "    |   Paymaster Address: {paymaster_address:?}")?;
                let paymaster_input_str = if paymaster_input.is_empty() {
                    "None".to_string()
                } else {
                    paymaster_input.encode_hex()
                };
                writeln!(w, "    |   Paymaster Input: 0x{paymaster_input_str}")?;
            }
        }
        Ok(())
    }
}

impl Display for PrettyTransactionEstimationView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}
