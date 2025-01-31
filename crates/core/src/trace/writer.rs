//! Helper methods to display transaction data in more human readable way.

////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Attribution: File adapted from the `revm-inspectors`crate for zksync usage                             //
//                                                                                                        //
// Full credit goes to its authors. See the original implementation here:                                 //
// https://github.com/paradigmxyz/revm-inspectors/blob/main/src/tracing/writer.rs                         //
//                                                                                                        //
// Note: These methods are used under the terms of the original project's license.                        //
////////////////////////////////////////////////////////////////////////////////////////////////////////////

use super::types::{CallLog, TraceMemberOrder};
use crate::trace::types::{
    CallTrace, CallTraceArena, CallTraceNode, DecodedCallData, ExecutionResultDisplay,
};
use anstyle::{AnsiColor, Color, Style};
use colorchoice::ColorChoice;
use hex::encode;
use std::io::{self, Write};
use std::str;
use zksync_multivm::interface::CallType;
use zksync_types::{
    zk_evm_types::FarCallOpcode,
};

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
    write_bytecodes: bool,    
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
            write_bytecodes: false,
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
        let root_node = &arena.arena[0];
        for &child_idx in &root_node.children {
            self.write_node(arena.nodes(), child_idx)?;
        }
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
                self.write_log(&node.logs[*index])?;
                Ok(item_idx + 1)
            }
            TraceMemberOrder::Call(index) => {
                assert!(*index < node.children.len(), "Call index out of bounds");
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
        let address = format!("0x{}", encode(trace.call.to));

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
                        FarCallOpcode::Normal => None,
                        FarCallOpcode::Delegate => Some(" [delegatecall]"),
                        FarCallOpcode::Mimic => Some(" [mimiccall]"),
                    },
                    CallType::NearCall => Some("[nearcall]"), // TODO check if this is correct
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

    fn write_log(&mut self, log: &CallLog) -> io::Result<()> {
        let log_style = self.log_style();
        self.write_branch()?;

        if let Some(name) = &log.decoded.name {
            write!(self.writer, "emit {name}({log_style}")?;
            if let Some(params) = &log.decoded.params {
                for (i, (param_name, value)) in params.iter().enumerate() {
                    if i > 0 {
                        self.writer.write_all(b", ")?;
                    }
                    write!(self.writer, "{param_name}: {value}")?;
                }
            }
            writeln!(self.writer, "{log_style:#})")?;
        } else {
            for (i, topic) in log.raw_log.indexed_topics.iter().enumerate() {
                if i == 0 {
                    self.writer.write_all(b" emit topic 0")?;
                } else {
                    self.write_pipes()?;
                    write!(self.writer, "       topic {i}")?;
                }
                writeln!(self.writer, ": {log_style}{topic}{log_style:#}")?;
            }
            if !log.raw_log.indexed_topics.is_empty() {
                self.write_pipes()?;
            }
            writeln!(
                self.writer,
                "          data: {log_style}{data}{log_style:#}",
                data = encode(&log.raw_log.value)
            )?;
        }
        Ok(())
    }

    /// Writes the footer of a call trace.
    fn write_trace_footer(&mut self, trace: &CallTrace) -> io::Result<()> {
        // Use the custom trait to format the execution result
        let status_str = trace.execution_result.result.display();

        // Write the execution result status using the formatted string
        write!(
            self.writer,
            "{style}{RETURN}[{status}]{style:#}",
            style = self.trace_style(trace),
            status = status_str,
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
        let color = if trace.success {
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
