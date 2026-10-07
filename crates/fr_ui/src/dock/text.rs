//! The dock layout as text, so a layout can be kept between runs.
//!
//! The format is a small JSON document written and read by hand, because a
//! layout is a handful of nodes and does not warrant a serialisation crate:
//!
//! ```text
//! {"version":1,"root":{"split":"horizontal","share":0.25,
//!   "a":{"tabs":["scene","assets"],"active":0},"b":{"open":true}}}
//! ```
//!
//! [`DockTree::to_text`] always writes the same text for the same tree, and
//! [`DockTree::from_text`] reads back exactly that tree. Anything else is an
//! error: unknown keys, a version other than the one this build writes, shares
//! outside their limits, empty or repeated panel names, a tab that does not
//! exist, and anything after the document.

use std::fmt;

use crate::dock::tree::{DockNode, DockTree};
use crate::style::Axis;

/// The version of the format this build writes and reads.
const VERSION: u32 = 1;

/// The deepest nesting of splits accepted.
const MAX_DEPTH: usize = 32;

/// Why a text is not a dock layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockTextError {
    /// What is wrong.
    message: String,
    /// The character offset in the text the problem was found at.
    offset: usize,
}

impl DockTextError {
    /// An error with `message` found at `offset`.
    fn at(offset: usize, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            offset,
        }
    }

    /// The character offset in the text the problem was found at.
    pub fn offset(&self) -> usize {
        self.offset
    }
}

impl fmt::Display for DockTextError {
    /// The problem and where it was found.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at offset {}", self.message, self.offset)
    }
}

impl std::error::Error for DockTextError {}

/// A value of the small JSON subset the format uses.
#[derive(Debug)]
enum Value {
    /// An object, its members in the order written.
    Object(Vec<(String, Value)>),
    /// An array.
    Array(Vec<Value>),
    /// A string.
    Text(String),
    /// A number, as the digits it was written with.
    Number(String),
    /// A boolean.
    Flag(bool),
}

/// A reader over the characters of a document.
struct Reader {
    /// The characters.
    chars: Vec<char>,
    /// The index of the next one.
    at: usize,
}

impl Reader {
    /// The next character, without taking it.
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    /// Skips whitespace.
    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.at += 1;
        }
    }

    /// Takes `expected`, or fails.
    fn expect(&mut self, expected: char) -> Result<(), DockTextError> {
        self.skip_space();
        if self.peek() == Some(expected) {
            self.at += 1;
            Ok(())
        } else {
            Err(DockTextError::at(self.at, format!("expected '{expected}'")))
        }
    }

    /// Takes the keyword `word`, or fails.
    fn keyword(&mut self, word: &str) -> Result<(), DockTextError> {
        for expected in word.chars() {
            if self.peek() != Some(expected) {
                return Err(DockTextError::at(self.at, format!("expected '{word}'")));
            }
            self.at += 1;
        }
        Ok(())
    }

    /// Reads one value, `depth` levels down.
    fn value(&mut self, depth: usize) -> Result<Value, DockTextError> {
        if depth > MAX_DEPTH * 2 {
            return Err(DockTextError::at(self.at, "nested too deeply"));
        }
        self.skip_space();
        match self.peek() {
            Some('{') => self.object(depth),
            Some('[') => self.array(depth),
            Some('"') => self.string().map(Value::Text),
            Some('t') => self.keyword("true").map(|()| Value::Flag(true)),
            Some('f') => self.keyword("false").map(|()| Value::Flag(false)),
            Some(c) if c == '-' || c.is_ascii_digit() => Ok(self.number()),
            _ => Err(DockTextError::at(self.at, "expected a value")),
        }
    }

    /// Reads an object.
    fn object(&mut self, depth: usize) -> Result<Value, DockTextError> {
        self.expect('{')?;
        let mut members = Vec::new();
        self.skip_space();
        if self.peek() == Some('}') {
            self.at += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.skip_space();
            let key = self.string()?;
            self.expect(':')?;
            members.push((key, self.value(depth + 1)?));
            self.skip_space();
            match self.peek() {
                Some(',') => self.at += 1,
                Some('}') => {
                    self.at += 1;
                    return Ok(Value::Object(members));
                }
                _ => return Err(DockTextError::at(self.at, "expected ',' or '}'")),
            }
        }
    }

    /// Reads an array.
    fn array(&mut self, depth: usize) -> Result<Value, DockTextError> {
        self.expect('[')?;
        let mut items = Vec::new();
        self.skip_space();
        if self.peek() == Some(']') {
            self.at += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_space();
            match self.peek() {
                Some(',') => self.at += 1,
                Some(']') => {
                    self.at += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(DockTextError::at(self.at, "expected ',' or ']'")),
            }
        }
    }

    /// Reads a string with its escapes.
    fn string(&mut self) -> Result<String, DockTextError> {
        self.skip_space();
        if self.peek() != Some('"') {
            return Err(DockTextError::at(self.at, "expected a string"));
        }
        self.at += 1;
        let mut text = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(DockTextError::at(self.at, "unterminated string"));
            };
            self.at += 1;
            match c {
                '"' => return Ok(text),
                '\\' => text.push(self.escape()?),
                c if c.is_control() => {
                    return Err(DockTextError::at(
                        self.at - 1,
                        "control character in string",
                    ));
                }
                c => text.push(c),
            }
        }
    }

    /// Reads the character after a backslash.
    fn escape(&mut self) -> Result<char, DockTextError> {
        let Some(c) = self.peek() else {
            return Err(DockTextError::at(self.at, "unterminated escape"));
        };
        self.at += 1;
        match c {
            '"' | '\\' | '/' => Ok(c),
            'n' => Ok('\n'),
            't' => Ok('\t'),
            'r' => Ok('\r'),
            'u' => {
                let digits: String = self.chars.iter().skip(self.at).take(4).collect();
                let code = u32::from_str_radix(&digits, 16)
                    .ok()
                    .filter(|_| digits.len() == 4)
                    .and_then(char::from_u32)
                    .ok_or_else(|| DockTextError::at(self.at, "bad unicode escape"))?;
                self.at += 4;
                Ok(code)
            }
            _ => Err(DockTextError::at(self.at - 1, "unknown escape")),
        }
    }

    /// Reads the characters of a number.
    fn number(&mut self) -> Value {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))
        {
            self.at += 1;
        }
        Value::Number(self.chars[start..self.at].iter().collect())
    }
}

/// The members of `value` as an object with exactly the keys `keys`, in any order.
fn members<'a>(
    value: &'a Value,
    keys: &[&str],
    offset: usize,
) -> Result<Vec<&'a Value>, DockTextError> {
    let Value::Object(found) = value else {
        return Err(DockTextError::at(offset, "expected an object"));
    };
    if found.len() != keys.len() {
        return Err(DockTextError::at(
            offset,
            format!("expected exactly {keys:?}"),
        ));
    }
    keys.iter()
        .map(|key| {
            found
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, member)| member)
                .ok_or_else(|| DockTextError::at(offset, format!("missing '{key}'")))
        })
        .collect()
}

/// The text of a string value.
fn text_of(value: &Value) -> Result<&str, DockTextError> {
    match value {
        Value::Text(text) => Ok(text),
        _ => Err(DockTextError::at(0, "expected a string")),
    }
}

/// The share written in a number value.
fn share_of(value: &Value) -> Result<f32, DockTextError> {
    match value {
        Value::Number(digits) => digits
            .parse::<f32>()
            .ok()
            .filter(|share| share.is_finite())
            .ok_or_else(|| DockTextError::at(0, "bad number")),
        _ => Err(DockTextError::at(0, "expected a number")),
    }
}

/// The index written in a number value.
fn index_of(value: &Value) -> Result<usize, DockTextError> {
    match value {
        Value::Number(digits) => digits
            .parse::<usize>()
            .map_err(|_| DockTextError::at(0, "expected a whole number")),
        _ => Err(DockTextError::at(0, "expected a number")),
    }
}

/// Reads the node `value` describes, `depth` splits down.
fn node_of(value: &Value, depth: usize) -> Result<DockNode, DockTextError> {
    if depth > MAX_DEPTH {
        return Err(DockTextError::at(0, "nested too deeply"));
    }
    let Value::Object(found) = value else {
        return Err(DockTextError::at(0, "a node must be an object"));
    };
    let kind = found
        .first()
        .map(|(name, _)| name.as_str())
        .ok_or_else(|| DockTextError::at(0, "empty node"))?;
    match kind {
        "split" => {
            let parts = members(value, &["split", "share", "a", "b"], 0)?;
            let axis = match text_of(parts[0])? {
                "horizontal" => Axis::Horizontal,
                "vertical" => Axis::Vertical,
                other => return Err(DockTextError::at(0, format!("unknown axis '{other}'"))),
            };
            Ok(DockNode::split(
                axis,
                share_of(parts[1])?,
                node_of(parts[2], depth + 1)?,
                node_of(parts[3], depth + 1)?,
            ))
        }
        "tabs" => {
            let parts = members(value, &["tabs", "active"], 0)?;
            let Value::Array(items) = parts[0] else {
                return Err(DockTextError::at(0, "tabs must be an array"));
            };
            let ids = items
                .iter()
                .map(|item| text_of(item).map(str::to_owned))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(DockNode::Tabs {
                ids,
                active: index_of(parts[1])?,
            })
        }
        "open" => {
            let parts = members(value, &["open"], 0)?;
            match parts[0] {
                Value::Flag(true) => Ok(DockNode::Open),
                _ => Err(DockTextError::at(0, "open must be true")),
            }
        }
        other => Err(DockTextError::at(0, format!("unknown node '{other}'"))),
    }
}

/// Writes `text` as a JSON string.
fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Writes `node` and everything below it.
fn write_node(out: &mut String, node: &DockNode) {
    match node {
        DockNode::Split { axis, share, a, b } => {
            let axis = match axis {
                Axis::Horizontal => "horizontal",
                Axis::Vertical => "vertical",
            };
            out.push_str(&format!("{{\"split\":\"{axis}\",\"share\":{share},\"a\":"));
            write_node(out, a);
            out.push_str(",\"b\":");
            write_node(out, b);
            out.push('}');
        }
        DockNode::Tabs { ids, active } => {
            out.push_str("{\"tabs\":[");
            for (index, id) in ids.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_string(out, id);
            }
            out.push_str(&format!("],\"active\":{active}}}"));
        }
        DockNode::Open => out.push_str("{\"open\":true}"),
    }
}

impl DockTree {
    /// The layout as text, the same text for the same layout.
    pub fn to_text(&self) -> String {
        let mut out = format!("{{\"version\":{VERSION},\"root\":");
        write_node(&mut out, self.root());
        out.push('}');
        out
    }

    /// The layout written in `text` by [`DockTree::to_text`].
    ///
    /// # Errors
    ///
    /// Returns an error when `text` is not a layout this build wrote: not the
    /// format, another version, an unsound tree, or text after the document.
    pub fn from_text(text: &str) -> Result<Self, DockTextError> {
        let mut reader = Reader {
            chars: text.chars().collect(),
            at: 0,
        };
        let document = reader.value(0)?;
        reader.skip_space();
        if reader.peek().is_some() {
            return Err(DockTextError::at(reader.at, "text after the document"));
        }
        let parts = members(&document, &["version", "root"], 0)?;
        match parts[0] {
            Value::Number(digits) if digits == &VERSION.to_string() => {}
            Value::Number(digits) => {
                return Err(DockTextError::at(
                    0,
                    format!("layout version {digits} found, {VERSION} required"),
                ));
            }
            _ => return Err(DockTextError::at(0, "version must be a number")),
        }
        let tree = Self::raw(node_of(parts[1], 0)?);
        if !tree.is_valid() {
            return Err(DockTextError::at(0, "the layout is not sound"));
        }
        Ok(tree)
    }
}
