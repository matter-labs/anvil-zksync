use anyhow::Context;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

/// Writes the given serializable object as JSON to the specified file path using pretty printing.
/// Returns an error if the file cannot be created or if serialization/writing fails.
pub fn write_json_file<T: Serialize>(path: &Path, obj: &T) -> anyhow::Result<()> {
    let file = File::create(path)
        .with_context(|| format!("Failed to create file '{}'", path.display()))?;
    let mut writer = BufWriter::new(file);
    // Note: intentionally using pretty printing for better readability.
    serde_json::to_writer_pretty(&mut writer, obj)
        .with_context(|| format!("Failed to write JSON to '{}'", path.display()))?;
    writer
        .flush()
        .with_context(|| format!("Failed to flush writer for '{}'", path.display()))?;

    Ok(())
}

/// Reads the JSON file at the specified path and deserializes it into the provided type.
/// Returns an error if the file cannot be read or deserialization fails.
pub fn read_json_file<T: DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let file_content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file '{}'", path.display()))?;

    serde_json::from_str(&file_content)
        .with_context(|| format!("Failed to deserialize JSON from '{}'", path.display()))
}
