//! The `key = value` text format of the settings files.

/// Reads `key = value` lines into pairs in file order. Blank lines and lines
/// starting with `#` are skipped, as are lines without an `=`. A key may repeat.
pub(super) fn parse_ini(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .collect()
}

/// Writes pairs as `key = value` lines, each ending with a newline.
pub(super) fn format_ini<'a>(entries: impl IntoIterator<Item = (&'a str, String)>) -> String {
    entries
        .into_iter()
        .map(|(key, value)| format!("{key} = {value}\n"))
        .collect()
}

/// The value of the first pair with the key.
pub(super) fn first_value<'a>(entries: &'a [(String, String)], key: &str) -> Option<&'a str> {
    entries
        .iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value.as_str())
}

/// The values of every pair with the key, in file order.
pub(super) fn all_values<'a>(
    entries: &'a [(String, String)],
    key: &'a str,
) -> impl Iterator<Item = &'a str> {
    entries
        .iter()
        .filter(move |(candidate, _)| candidate == key)
        .map(|(_, value)| value.as_str())
}

/// Reads a file as text, none when it does not exist.
///
/// # Errors
///
/// The reason, as text, when the file exists and cannot be read.
pub(super) fn read_optional(path: &std::path::Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

/// Writes a file atomically with `fr_document::write_file`.
///
/// # Errors
///
/// The reason, as text, when the file cannot be written.
pub(super) fn write_text(path: &std::path::Path, text: &str) -> Result<(), String> {
    fr_document::write_file(path, text.as_bytes()).map_err(|error| error.to_string())
}
