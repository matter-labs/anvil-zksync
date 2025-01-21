//! Helper methods to display transaction data in more human readable way.
// use crate::bootloader_debug::BootloaderDebug;
// use crate::resolver;
// use crate::utils::block_on;
// use crate::utils::{calculate_eth_cost, to_human_size};
// use anvil_zksync_config::utils::format_gwei;
// use anvil_zksync_types::ShowCalls;
// use colored::Colorize;
// use futures::future::join_all;
// use lazy_static::lazy_static;
// use serde::Deserialize;
use crate::trace::types::{
    CallTrace, CallTraceArena, CallTraceNode, DecodedCallData, DecodedCallTrace,
};
use anstyle::{AnsiColor, Color, Style};
use colorchoice::ColorChoice;
use std::io::{self, Write};
use std::str;
use zksync_multivm::interface::{CallType, VmExecutionLogs, VmEvent};
use zksync_types::{
    StorageLogWithPreviousValue,
    zk_evm_types::FarCallOpcode,
    l2_to_l1_log::{UserL2ToL1Log, SystemL2ToL1Log},
    H256,
    //fee_model::FeeModelConfigV2, Address, StorageLogWithPreviousValue, Transaction, H160, H256,
    U256,
};

use super::types::TraceMemberOrder;

const PIPE: &str = "  │ ";
const EDGE: &str = "  └─ ";
const BRANCH: &str = "  ├─ ";
const CALL: &str = "→ ";
const RETURN: &str = "← ";

const TRACE_KIND_STYLE: Style = AnsiColor::Yellow.on_default();
const LOG_STYLE: Style = AnsiColor::Cyan.on_default();

/// Configuration for a [`TraceWriter`].
#[derive(Clone, Debug)]
#[allow(missing_copy_implementations)]
pub struct TraceWriterConfig {
    use_colors: bool,
    color_cheatcodes: bool,
    write_bytecodes: bool,
    write_storage_changes: bool,
}

impl Default for TraceWriterConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceWriterConfig {
    /// Create a new `TraceWriterConfig` with default settings.
    pub fn new() -> Self {
        Self {
            use_colors: use_colors(ColorChoice::Auto),
            color_cheatcodes: false,
            write_bytecodes: false,
            write_storage_changes: false,
        }
    }

    /// Use colors in the output. Default: [`Auto`](ColorChoice::Auto).
    pub fn color_choice(mut self, choice: ColorChoice) -> Self {
        self.use_colors = use_colors(choice);
        self
    }

    /// Get the current color choice. `Auto` is lost, so this returns `true` if colors are enabled.
    pub fn get_use_colors(&self) -> bool {
        self.use_colors
    }

    /// Color calls to the cheatcode address differently. Default: false.
    pub fn color_cheatcodes(mut self, yes: bool) -> Self {
        self.color_cheatcodes = yes;
        self
    }

    /// Returns `true` if calls to the cheatcode address are colored differently.
    pub fn get_color_cheatcodes(&self) -> bool {
        self.color_cheatcodes
    }

    /// Write contract creation codes and deployed codes when writing "create" traces.
    /// Default: false.
    pub fn write_bytecodes(mut self, yes: bool) -> Self {
        self.write_bytecodes = yes;
        self
    }

    /// Returns `true` if contract creation codes and deployed codes are written.
    pub fn get_write_bytecodes(&self) -> bool {
        self.write_bytecodes
    }

    // /// Sets whether to write storage changes.
    // pub fn write_storage_changes(mut self, yes: bool) -> Self {
    //     self.write_storage_changes = yes;
    //     self
    // }

    // /// Returns `true` if storage changes are written to the writer.
    // pub fn get_write_storage_changes(&self) -> bool {
    //     self.write_storage_changes
    // }
}

/// Formats [call traces](CallTraceArena) to an [`Write`] writer.
///
/// Will never write invalid UTF-8.
#[derive(Clone, Debug)]
pub struct TraceWriter<W> {
    writer: W,
    indentation_level: u16,
    config: TraceWriterConfig,
}

impl<W: Write> TraceWriter<W> {
    /// Create a new `TraceWriter` with the given writer.
    #[inline]
    pub fn new(writer: W) -> Self {
        Self::with_config(writer, TraceWriterConfig::new())
    }

    /// Create a new `TraceWriter` with the given writer and configuration.
    pub fn with_config(writer: W, config: TraceWriterConfig) -> Self {
        Self {
            writer,
            indentation_level: 0,
            config,
        }
    }

    /// Sets the color choice.
    #[inline]
    pub fn use_colors(mut self, color_choice: ColorChoice) -> Self {
        self.config.use_colors = use_colors(color_choice);
        self
    }

    /// Sets whether to color calls to the cheatcode address differently.
    #[inline]
    pub fn color_cheatcodes(mut self, yes: bool) -> Self {
        self.config.color_cheatcodes = yes;
        self
    }

    /// Sets the starting indentation level.
    #[inline]
    pub fn with_indentation_level(mut self, level: u16) -> Self {
        self.indentation_level = level;
        self
    }

    /// Sets whether contract creation codes and deployed codes should be written.
    #[inline]
    pub fn write_bytecodes(mut self, yes: bool) -> Self {
        self.config.write_bytecodes = yes;
        self
    }

    /// Sets whether to write storage changes.
    #[inline]
    pub fn with_storage_changes(mut self, yes: bool) -> Self {
        self.config.write_storage_changes = yes;
        self
    }

    /// Returns a reference to the inner writer.
    #[inline]
    pub const fn writer(&self) -> &W {
        &self.writer
    }

    /// Returns a mutable reference to the inner writer.
    #[inline]
    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Consumes the `TraceWriter` and returns the inner writer.
    #[inline]
    pub fn into_writer(self) -> W {
        self.writer
    }

    /// Writes a call trace arena to the writer.
    pub fn write_arena(&mut self, arena: &CallTraceArena) -> io::Result<()> {
        self.write_node(arena.nodes(), 0)?;
        self.writer.flush()
    }

    /// Writes a single item of a single node to the writer. Returns the index of the next item to
    /// be written.
    ///
    /// Note: this will return length of [CallTraceNode::ordering] when last item will get
    /// processed.
    /// Writes a single item of a single node to the writer. Returns the index of the next item to
    /// be written.
    ///
    /// Note: this will return the length of [CallTraceNode::ordering] when the last item gets
    /// processed.
    fn write_item(
        &mut self,
        nodes: &[CallTraceNode],
        node_idx: usize,
        item_idx: usize,
    ) -> io::Result<usize> {
        let node = &nodes[node_idx];
        match &node.ordering[item_idx] {
            TraceMemberOrder::Log(index) => {
                let logs = &node.trace.execution_result.logs;

                if *index < logs.storage_logs.len() {
                    self.write_storage_log(&logs.storage_logs[*index])?;
                } else if *index < logs.storage_logs.len() + logs.events.len() {
                    let event_index = *index - logs.storage_logs.len();
                    self.write_event_log(&logs.events[event_index])?;
                } else if *index
                    < logs.storage_logs.len() + logs.events.len() + logs.user_l2_to_l1_logs.len()
                {
                    let user_log_index = *index - logs.storage_logs.len() - logs.events.len();
                    self.write_user_l2_to_l1_log(&logs.user_l2_to_l1_logs[user_log_index])?;
                } else if *index
                    < logs.storage_logs.len()
                        + logs.events.len()
                        + logs.user_l2_to_l1_logs.len()
                        + logs.system_l2_to_l1_logs.len()
                {
                    let system_log_index = *index
                        - logs.storage_logs.len()
                        - logs.events.len()
                        - logs.user_l2_to_l1_logs.len();
                    self.write_system_l2_to_l1_log(&logs.system_l2_to_l1_logs[system_log_index])?;
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Log index out of bounds",
                    ));
                }

                Ok(item_idx + 1)
            }
            TraceMemberOrder::Call(index) => {
                self.write_node(nodes, node.children[*index])?;
                Ok(item_idx + 1)
            }
        }
    }

    /// Writes items of a single node to the writer, starting from the given index, and until the
    /// given predicate is false.
    ///
    /// Returns the index of the next item to be written.
    fn write_items_until(
        &mut self,
        nodes: &[CallTraceNode],
        node_idx: usize,
        first_item_idx: usize,
        f: impl Fn(usize) -> bool,
    ) -> io::Result<usize> {
        let mut item_idx = first_item_idx;
        while !f(item_idx) {
            item_idx = self.write_item(nodes, node_idx, item_idx)?;
        }
        Ok(item_idx)
    }

    /// Writes all items of a single node to the writer.
    fn write_items(&mut self, nodes: &[CallTraceNode], node_idx: usize) -> io::Result<()> {
        let items_cnt = nodes[node_idx].ordering.len();
        self.write_items_until(nodes, node_idx, 0, |idx| idx == items_cnt)?;
        Ok(())
    }

    /// Writes a single node and its children to the writer.
    fn write_node(&mut self, nodes: &[CallTraceNode], idx: usize) -> io::Result<()> {
        let node = &nodes[idx];

        // Write header.
        self.write_branch()?;
        self.write_trace_header(&node.trace)?;
        self.writer.write_all(b"\n")?;

        // Write logs and subcalls.
        self.indentation_level += 1;
        self.write_items(nodes, idx)?;

        // if self.config.write_storage_changes {
        //     self.write_storage_changes(node)?;
        // }

        // Write return data.
        self.write_edge()?;
        self.write_trace_footer(&node.trace)?;
        self.writer.write_all(b"\n")?;

        self.indentation_level -= 1;

        Ok(())
    }

    /// Writes the header of a call trace.
    fn write_trace_header(&mut self, trace: &CallTrace) -> io::Result<()> {
        write!(self.writer, "[{}] ", trace.call.gas_used)?;

        let trace_kind_style = self.trace_kind_style();
        let address = trace.call.to.to_string();

        match trace.call.r#type {
            CallType::Create => {
                write!(
                    self.writer,
                    "{trace_kind_style}{CALL}new{trace_kind_style:#} {label}@{address}",
                    label = trace.decoded.label.as_deref().unwrap_or("<unknown>")
                )?;
                if self.config.write_bytecodes {
                    write!(self.writer, "({})", hex::encode(&trace.call.input))?;
                }
            }
            CallType::Call(_) | CallType::NearCall => {
                let (func_name, inputs) = match &trace.decoded.call_data {
                    Some(DecodedCallData { signature, args }) => {
                        let name = signature.split('(').next().unwrap();
                        (name.to_string(), args.join(", "))
                    }
                    None => {
                        if trace.call.input.len() < 4 {
                            ("fallback".to_string(), hex::encode(&trace.call.input))
                        } else {
                            let (selector, data) = trace.call.input.split_at(4);
                            (hex::encode(selector), hex::encode(data))
                        }
                    }
                };

                write!(
                    self.writer,
                    "{style}{addr}{style:#}::{style}{func_name}{style:#}",
                    style = self.trace_style(trace),
                    addr = trace.decoded.label.as_deref().unwrap_or(&address),
                )?;

                if !trace.call.value.is_zero() {
                    write!(self.writer, "{{value: {}}}", trace.call.value)?;
                }

                write!(self.writer, "({inputs})")?;

                let action = match trace.call.r#type {
                    CallType::Call(opcode) => match opcode {
                        FarCallOpcode::Normal => None, // No additional action for Normal calls.
                        FarCallOpcode::Delegate => Some(" [delegatecall]"),
                        FarCallOpcode::Mimic => Some(" [mimiccall]"), // Handle Mimic calls.
                    },
                    CallType::NearCall => None, // No additional action for NearCall.
                    CallType::Create => unreachable!(), // Create calls are handled separately.
                };

                if let Some(action) = action {
                    write!(
                        self.writer,
                        "{trace_kind_style}{action}{trace_kind_style:#}"
                    )?;
                }
            }
        }

        Ok(())
    }

    fn write_storage_log(&mut self, log: &StorageLogWithPreviousValue) -> io::Result<()> {
        let log_style = self.log_style();
        self.write_branch()?;
        writeln!(
        self.writer,
        "{log_style}StorageLog: Key: {key:?}, PrevValue: {prev:?}, NewValue: {new:?}, Kind: {kind:?}",
        key = log.log.key,
        prev = log.previous_value,
        new = log.log.value,
        kind = log.log.kind
    )
    }

    fn write_event_log(&mut self, log: &VmEvent) -> io::Result<()> {
        let log_style = self.log_style();
        self.write_branch()?;
        writeln!(
        self.writer,
        "{log_style}EventLog: Location: {location:?}, Address: {address}, Topics: {topics:?}, Data: {data}",
        location = log.location,
        address = log.address,
        topics = log.indexed_topics,
        data = hex::encode(&log.value)
    )
    }

    fn write_user_l2_to_l1_log(&mut self, log: &UserL2ToL1Log) -> io::Result<()> {
        let log_style = self.log_style();
        self.write_branch()?;
        writeln!(
        self.writer,
        "{log_style}User L2->L1 Log: ShardID: {shard}, Tx#: {tx_num}, Sender: {sender}, Key: {key}, Value: {value}",
        shard = log.0.shard_id,
        tx_num = log.0.tx_number_in_block,
        sender = log.0.sender,
        key = log.0.key,
        value = log.0.value
    )
    }

    fn write_system_l2_to_l1_log(&mut self, log: &SystemL2ToL1Log) -> io::Result<()> {
        let log_style = self.log_style();
        self.write_branch()?;
        writeln!(
        self.writer,
        "{log_style}System L2->L1 Log: ShardID: {shard}, Tx#: {tx_num}, Sender: {sender}, Key: {key}, Value: {value}",
        shard = log.0.shard_id,
        tx_num = log.0.tx_number_in_block,
        sender = log.0.sender,
        key = log.0.key,
        value = log.0.value
    )
    }

    fn write_log(&mut self, logs: &VmExecutionLogs) -> io::Result<()> {
        let log_style = self.log_style();

        // Write storage logs.
        for (i, storage_log) in logs.storage_logs.iter().enumerate() {
            self.write_branch()?;
            writeln!(
            self.writer,
            "{log_style}StorageLog #{i}{log_style:#}: Key: {key:?}, PrevValue: {prev}, NewValue: {new}, Kind: {kind:?}",
            key = storage_log.log.key,
            prev = storage_log.previous_value,
            new = storage_log.log.value,
            kind = storage_log.log.kind
        )?;
        }

        // Write user L2->L1 logs.
        for (i, user_log) in logs.user_l2_to_l1_logs.iter().enumerate() {
            self.write_branch()?;
            writeln!(
            self.writer,
            "{log_style}User L2->L1 Log #{i}{log_style:#}: ShardID: {shard}, Tx#: {tx_num}, Sender: {sender}, Key: {key}, Value: {value}",
            shard = user_log.0.shard_id,
            tx_num = user_log.0.tx_number_in_block,
            sender = user_log.0.sender,
            key = user_log.0.key,
            value = user_log.0.value
        )?;
        }

        // Write system L2->L1 logs.
        for (i, system_log) in logs.system_l2_to_l1_logs.iter().enumerate() {
            self.write_branch()?;
            writeln!(
            self.writer,
            "{log_style}System L2->L1 Log #{i}{log_style:#}: ShardID: {shard}, Tx#: {tx_num}, Sender: {sender}, Key: {key}, Value: {value}",
            shard = system_log.0.shard_id,
            tx_num = system_log.0.tx_number_in_block,
            sender = system_log.0.sender,
            key = system_log.0.key,
            value = system_log.0.value
        )?;
        }

        // Write event logs.
        for (i, event) in logs.events.iter().enumerate() {
            self.write_branch()?;
            writeln!(
            self.writer,
            "{log_style}Event Log #{i}{log_style:#}: Location: {location:?}, Address: {address}, Topics: {topics:?}, Data: {data}",
            location = event.location,
            address = event.address,
            topics = event.indexed_topics,
            data = hex::encode(&event.value)
        )?;
        }

        Ok(())
    }

    /// Writes the footer of a call trace.
    fn write_trace_footer(&mut self, trace: &CallTrace) -> io::Result<()> {
        // Write the execution result status
        write!(
            self.writer,
            "{style}{RETURN}[{status:?}]{style:#}",
            style = self.trace_style(trace),
            status = trace.execution_result.result,
        )?;

        // Write decoded return data if available
        if let Some(decoded) = &trace.decoded.return_data {
            write!(self.writer, " ")?;
            return self.writer.write_all(decoded.as_bytes());
        }

        // Handle contract creation or output data
        if !self.config.write_bytecodes
            && matches!(trace.call.r#type, CallType::Create)
            && !trace.execution_result.result.is_failed()
        {
            write!(self.writer, " {} bytes of code", trace.call.output.len())?;
        } else if !trace.call.output.is_empty() {
            write!(self.writer, " {}", hex::encode(&trace.call.output))?;
        }

        Ok(())
    }

    fn write_indentation(&mut self) -> io::Result<()> {
        self.writer.write_all(b"  ")?;
        for _ in 1..self.indentation_level {
            self.writer.write_all(PIPE.as_bytes())?;
        }
        Ok(())
    }

    #[doc(alias = "left_prefix")]
    fn write_branch(&mut self) -> io::Result<()> {
        self.write_indentation()?;
        if self.indentation_level != 0 {
            self.writer.write_all(BRANCH.as_bytes())?;
        }
        Ok(())
    }

    #[doc(alias = "right_prefix")]
    fn write_pipes(&mut self) -> io::Result<()> {
        self.write_indentation()?;
        self.writer.write_all(PIPE.as_bytes())
    }

    fn write_edge(&mut self) -> io::Result<()> {
        self.write_indentation()?;
        self.writer.write_all(EDGE.as_bytes())
    }

    fn trace_style(&self, trace: &CallTrace) -> Style {
        if !self.config.use_colors {
            return Style::default();
        }
        let color = if self.config.color_cheatcodes {
            AnsiColor::Blue
        } else if trace.success {
            AnsiColor::Green
        } else {
            AnsiColor::Red
        };
        Color::Ansi(color).on_default()
    }

    fn trace_kind_style(&self) -> Style {
        if !self.config.use_colors {
            return Style::default();
        }
        TRACE_KIND_STYLE
    }

    fn log_style(&self) -> Style {
        if !self.config.use_colors {
            return Style::default();
        }
        LOG_STYLE
    }
}

fn use_colors(choice: ColorChoice) -> bool {
    use io::IsTerminal;
    match choice {
        ColorChoice::Auto => io::stdout().is_terminal(),
        ColorChoice::AlwaysAnsi | ColorChoice::Always => true,
        ColorChoice::Never => false,
    }
}

// Formats the given U256 as a decimal number if it is short, otherwise as a hexadecimal
// byte-array.
// fn num_or_hex(x: U256) -> String {
//     if x < U256::from(1e6 as u128) {
//         x.to_string()
//     } else {
//         H256::from(x).to_string()
//     }
// }
