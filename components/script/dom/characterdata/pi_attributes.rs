/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Parsing and serialising the pseudo-attributes of a `ProcessingInstruction`'s data,
//! such as `type="text/css" href="style.css"`.
//!
//! The attribute list is always derived from the data, so nothing is stored on the node.

use crate::dom::bindings::domname::is_valid_attribute_local_name;

/// An attribute name and its (unescaped) value.
pub(crate) type PseudoAttribute = (String, String);

fn is_ascii_whitespace(c: char) -> bool {
    c.is_ascii_whitespace()
}

/// Parse the data of a processing instruction into its pseudo-attributes. Every value must be
/// double-quoted. If the data is not made only of such attributes, the result is empty. When a
/// name occurs more than once, only the first occurrence is kept.
pub(crate) fn parse(data: &str) -> Vec<PseudoAttribute> {
    parse_attributes(data).unwrap_or_default()
}

fn parse_attributes(data: &str) -> Option<Vec<PseudoAttribute>> {
    let mut attributes: Vec<PseudoAttribute> = Vec::new();
    let mut rest = data.trim_start_matches(is_ascii_whitespace);
    while !rest.is_empty() {
        let name_end = rest
            .find(|c: char| is_ascii_whitespace(c) || c == '=')
            .unwrap_or(rest.len());
        let (name, after_name) = rest.split_at(name_end);
        if !is_valid_attribute_local_name(name) {
            return None;
        }
        let after_equals = after_name
            .trim_start_matches(is_ascii_whitespace)
            .strip_prefix('=')?;
        let after_quote = after_equals
            .trim_start_matches(is_ascii_whitespace)
            .strip_prefix('"')?;
        let (value, after_value) = after_quote.split_once('"')?;
        if !attributes.iter().any(|(existing, _)| existing == name) {
            attributes.push((name.to_owned(), unescape(value)));
        }
        rest = after_value.trim_start_matches(is_ascii_whitespace);
    }
    Some(attributes)
}

/// Serialise attributes the way the HTML serialiser does for an element's start tag,
/// separating them with a single space.
pub(crate) fn serialize(attributes: &[PseudoAttribute]) -> String {
    let mut data = String::new();
    for (index, (name, value)) in attributes.iter().enumerate() {
        if index > 0 {
            data.push(' ');
        }
        data.push_str(name);
        data.push_str("=\"");
        escape_into(value, &mut data);
        data.push('"');
    }
    data
}

fn escape_into(value: &str, output: &mut String) {
    for c in value.chars() {
        match c {
            '&' => output.push_str("&amp;"),
            '"' => output.push_str("&quot;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '\u{A0}' => output.push_str("&nbsp;"),
            _ => output.push(c),
        }
    }
}

/// Resolve the character references that [`escape_into`] produces, along with numeric ones.
/// Anything that is not a known reference is left as it is.
fn unescape(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('&') {
        output.push_str(&rest[..start]);
        rest = &rest[start..];
        let resolved = rest.find(';').and_then(|end| {
            let reference = &rest[1..end];
            let c = match reference {
                "amp" => '&',
                "quot" => '"',
                "apos" => '\'',
                "lt" => '<',
                "gt" => '>',
                "nbsp" => '\u{A0}',
                _ => {
                    let digits = reference.strip_prefix('#')?;
                    let code_point = match digits.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => digits.parse().ok()?,
                    };
                    char::from_u32(code_point)?
                },
            };
            Some((c, end + 1))
        });
        match resolved {
            Some((c, length)) => {
                output.push(c);
                rest = &rest[length..];
            },
            None => {
                output.push('&');
                rest = &rest[1..];
            },
        }
    }
    output.push_str(rest);
    output
}
