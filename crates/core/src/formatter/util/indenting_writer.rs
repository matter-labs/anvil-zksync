use std::fmt::Write;

/// IndentingWriter wraps any Write and manages indentation at line starts.
///
/// This utility provides an easy way to generate indented text with prefixes
/// at the beginning of each line. It is useful for formatting output
/// such as debug information, code, or structured data.
///
/// # Examples
///
/// ```
/// use std::fmt::Write;
/// use anvil_zksync::formatter::util::IndentingWriter;
///
/// let mut output = String::new();
/// let mut writer = IndentingWriter::new(&mut output, Some("> "));
/// writer.indent();
/// writeln!(writer, "First line").unwrap();
/// writeln!(writer, "Second line").unwrap();
/// writer.dedent();
/// writeln!(writer, "Third line").unwrap();
/// ```
pub struct IndentingWriter<'a, W: Write> {
    writer: &'a mut W,
    indent: usize,
    prefix: Vec<&'a str>,
    newline: bool,
}

impl<'a, W: Write> IndentingWriter<'a, W> {
    const TABULATION: &'static str = "   ";

    /// Creates a new IndentingWriter that wraps the provided writer.
    ///
    /// # Arguments
    ///
    /// * `writer` - The underlying writer to which formatted text will be written
    /// * `prefix` - An optional prefix string to add at the beginning of each line after indentation
    pub fn new(writer: &'a mut W, prefix: Option<&'a str>) -> Self {
        Self {
            writer,
            indent: 0,
            prefix: prefix.into_iter().collect(),
            newline: true,
        }
    }

    /// Adds a new prefix to the stack of prefixes.
    ///
    /// The most recently added prefix will be used for new lines.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix string to add
    pub fn push_prefix(&mut self, prefix: &'static str) {
        self.prefix.push(prefix)
    }

    /// Removes the most recently added prefix from the stack.
    ///
    /// If the stack is empty, this operation does nothing.
    pub fn pop_prefix(&mut self) {
        let _ = self.prefix.pop();
    }

    /// Increases the indentation level by one.
    pub fn indent(&mut self) {
        self.indent += 1;
    }

    /// Decreases the indentation level by one.
    ///
    /// If the indentation level is already 0, this operation does nothing.
    pub fn dedent(&mut self) {
        if self.indent > 0 {
            self.indent -= 1;
        }
    }
}

impl<W: Write> IndentingWriter<'_, W> {
    /// Applies indentation and prefix to the current line if needed.
    ///
    /// This is automatically called before writing any content and handles:
    /// 1. Adding the appropriate number of tabs based on indentation level
    /// 2. Adding the current prefix (if any)
    ///
    /// This method is only applied at the beginning of a line (after a newline).
    fn do_indent(&mut self) -> std::fmt::Result {
        if self.newline {
            for _ in 0..self.indent {
                self.writer.write_str(Self::TABULATION)?;
            }
            self.writer.write_str(self.prefix.last().unwrap_or(&""))?;
            self.newline = false;
        }

        Ok(())
    }
}
impl<W: Write> Write for IndentingWriter<'_, W> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let mut start = 0;
        let mut chars = s.char_indices().peekable();

        while let Some((i, c)) = chars.next() {
            self.do_indent()?;
            if c == '\n' {
                self.newline = true;
                self.writer.write_str(&s[start..=i])?;
                start = i + c.len_utf8();
            } else if chars.peek().is_none() {
                // If it's the last chunk, write the remainder
                self.writer.write_str(&s[start..])?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiline_write() {
        let mut s = String::new();
        let mut iw = IndentingWriter::new(&mut s, Some("|"));

        iw.indent();

        write!(iw, " Hello \n\n world \n").unwrap();
        println!("{}", &s);

        assert_eq!(s, "   | Hello \n   |\n   | world \n")
    }
}
