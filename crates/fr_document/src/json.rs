//! JSON text over `serde_json`: parsing with diagnostics, deterministic
//! formatting and typed field readers that name what they expected.

use serde_json::{Map, Number, Value};

use crate::error::DocumentError;
use crate::guid::Guid;

/// Spaces of indentation per nesting level.
const INDENT: usize = 2;

/// Parses JSON text.
///
/// # Errors
///
/// [`DocumentError::Syntax`] with the line and column of the first problem.
pub fn parse(text: &str) -> Result<Value, DocumentError> {
    serde_json::from_str(text).map_err(|error| DocumentError::Syntax {
        message: error.to_string(),
    })
}

/// Parses UTF-8 bytes as JSON.
///
/// # Errors
///
/// [`DocumentError::NotUtf8`] with the offset of the first invalid byte, or the
/// syntax error of [`parse`].
pub fn parse_bytes(bytes: &[u8]) -> Result<Value, DocumentError> {
    let text = std::str::from_utf8(bytes).map_err(|error| DocumentError::NotUtf8 {
        offset: error.valid_up_to(),
    })?;
    parse(text)
}

/// Whether a value is written on one line.
fn is_scalar(value: &Value) -> bool {
    !matches!(value, Value::Array(_) | Value::Object(_))
}

/// Appends the indentation of a nesting level.
fn push_indent(out: &mut String, level: usize) {
    out.extend(std::iter::repeat_n(' ', level * INDENT));
}

/// Appends a scalar in its JSON text.
fn push_scalar(out: &mut String, value: &Value) {
    out.push_str(&value.to_string());
}

/// Appends a value at a nesting level: objects and arrays of containers span
/// lines, arrays of scalars stay on one.
fn push_value(out: &mut String, value: &Value, level: usize) {
    match value {
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Array(items) if items.iter().all(is_scalar) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                push_scalar(out, item);
            }
            out.push(']');
        }
        Value::Array(items) => {
            out.push_str("[\n");
            for (index, item) in items.iter().enumerate() {
                push_indent(out, level + 1);
                push_value(out, item, level + 1);
                out.push_str(if index + 1 < items.len() { ",\n" } else { "\n" });
            }
            push_indent(out, level);
            out.push(']');
        }
        Value::Object(map) => {
            out.push_str("{\n");
            for (index, (key, item)) in map.iter().enumerate() {
                push_indent(out, level + 1);
                push_scalar(out, &Value::String(key.clone()));
                out.push_str(": ");
                push_value(out, item, level + 1);
                out.push_str(if index + 1 < map.len() { ",\n" } else { "\n" });
            }
            push_indent(out, level);
            out.push('}');
        }
        scalar => push_scalar(out, scalar),
    }
}

/// Formats a value deterministically: keys in insertion order, two spaces of
/// indentation, scalar arrays on one line and a final newline.
pub fn write(value: &Value) -> String {
    let mut out = String::new();
    push_value(&mut out, value, 0);
    out.push('\n');
    out
}

/// A float as a JSON number that reads back as the same float: the shortest
/// decimal that identifies it.
pub fn number(value: f32) -> Value {
    let shortest = value.to_string().parse::<f64>().unwrap_or(0.0);
    Number::from_f64(shortest).map_or(Value::Null, Value::Number)
}

/// An empty object.
pub fn object() -> Map<String, Value> {
    Map::new()
}

/// A guid as its 32 digits.
pub fn guid_value(id: Guid) -> Value {
    Value::String(id.to_text())
}

/// The error of a value that is not what a reader expected.
fn expected(what: &str, context: &str) -> DocumentError {
    DocumentError::malformed(format!("{context}: expected {what}"))
}

/// A value as an object.
///
/// # Errors
///
/// When the value is not an object.
pub fn as_object<'a>(
    value: &'a Value,
    context: &str,
) -> Result<&'a Map<String, Value>, DocumentError> {
    value
        .as_object()
        .ok_or_else(|| expected("an object", context))
}

/// A value as an array.
///
/// # Errors
///
/// When the value is not an array.
pub fn as_array<'a>(value: &'a Value, context: &str) -> Result<&'a Vec<Value>, DocumentError> {
    value
        .as_array()
        .ok_or_else(|| expected("an array", context))
}

/// A value as a string.
///
/// # Errors
///
/// When the value is not a string.
pub fn as_str<'a>(value: &'a Value, context: &str) -> Result<&'a str, DocumentError> {
    value.as_str().ok_or_else(|| expected("a string", context))
}

/// A value as a float.
///
/// # Errors
///
/// When the value is not a number.
pub fn as_f32(value: &Value, context: &str) -> Result<f32, DocumentError> {
    value
        .as_f64()
        .map(|number| number as f32)
        .ok_or_else(|| expected("a number", context))
}

/// A value as an integer.
///
/// # Errors
///
/// When the value is not an integer.
pub fn as_i64(value: &Value, context: &str) -> Result<i64, DocumentError> {
    value
        .as_i64()
        .ok_or_else(|| expected("an integer", context))
}

/// A value as a boolean.
///
/// # Errors
///
/// When the value is not a boolean.
pub fn as_bool(value: &Value, context: &str) -> Result<bool, DocumentError> {
    value
        .as_bool()
        .ok_or_else(|| expected("a boolean", context))
}

/// A value as `N` floats.
///
/// # Errors
///
/// When the value is not an array of exactly `N` numbers.
pub fn as_floats<const N: usize>(value: &Value, context: &str) -> Result<[f32; N], DocumentError> {
    let items = as_array(value, context)?;
    if items.len() != N {
        return Err(expected(&format!("{N} numbers"), context));
    }
    let mut result = [0.0; N];
    for (slot, item) in result.iter_mut().zip(items) {
        *slot = as_f32(item, context)?;
    }
    Ok(result)
}

/// A value as a guid; the empty string is the unset guid.
///
/// # Errors
///
/// When the value is neither empty nor 32 hexadecimal digits.
pub fn as_guid(value: &Value, context: &str) -> Result<Guid, DocumentError> {
    let text = as_str(value, context)?;
    if text.is_empty() {
        return Ok(Guid::NONE);
    }
    Guid::from_text(text).ok_or_else(|| expected("32 hexadecimal digits", context))
}

/// A required field of an object.
///
/// # Errors
///
/// When the field is absent.
pub fn required<'a>(
    map: &'a Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a Value, DocumentError> {
    map.get(key)
        .ok_or_else(|| DocumentError::malformed(format!("{context}: missing \"{key}\"")))
}

/// A string field, empty when absent.
///
/// # Errors
///
/// When the field is present and not a string.
pub fn string_or_empty(
    map: &Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<String, DocumentError> {
    match map.get(key) {
        Some(value) => as_str(value, &format!("{context}.{key}")).map(str::to_owned),
        None => Ok(String::new()),
    }
}

/// A boolean field, a default when absent.
///
/// # Errors
///
/// When the field is present and not a boolean.
pub fn bool_or(
    map: &Map<String, Value>,
    key: &str,
    default: bool,
    context: &str,
) -> Result<bool, DocumentError> {
    match map.get(key) {
        Some(value) => as_bool(value, &format!("{context}.{key}")),
        None => Ok(default),
    }
}

/// An integer field, a default when absent.
///
/// # Errors
///
/// When the field is present and not an integer.
pub fn int_or(
    map: &Map<String, Value>,
    key: &str,
    default: i64,
    context: &str,
) -> Result<i64, DocumentError> {
    match map.get(key) {
        Some(value) => as_i64(value, &format!("{context}.{key}")),
        None => Ok(default),
    }
}

/// A guid field, unset when absent.
///
/// # Errors
///
/// When the field is present and not a guid.
pub fn guid_or_none(
    map: &Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<Guid, DocumentError> {
    match map.get(key) {
        Some(value) => as_guid(value, &format!("{context}.{key}")),
        None => Ok(Guid::NONE),
    }
}

/// The items of an array field, none when absent.
///
/// # Errors
///
/// When the field is present and not an array.
pub fn items_or_empty<'a>(
    map: &'a Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a [Value], DocumentError> {
    match map.get(key) {
        Some(value) => as_array(value, &format!("{context}.{key}")).map(Vec::as_slice),
        None => Ok(&[]),
    }
}
