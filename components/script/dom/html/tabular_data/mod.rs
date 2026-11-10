/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub(crate) mod htmltablecaptionelement;
pub(crate) mod htmltablecellelement;
pub(crate) mod htmltablecolelement;
pub(crate) mod htmltableelement;
pub(crate) mod htmltablerowelement;
pub(crate) mod htmltablesectionelement;

use html5ever::LocalName;
use js::context::JSContext;
use style::attr::AttrValue;

use crate::dom::element::Element;
use crate::dom::values::UNSIGNED_LONG_MAX;

/// Parse a table span's non-negative integer, retaining the content attribute's
/// original text while clamping its numeric value for DOM and layout consumers.
fn parse_clamped_span(value: String, min: u32, max: u32, default: u32) -> AttrValue {
    let input = value.trim_start_matches(['\t', '\n', '\x0c', '\r', ' ']);
    let (negative, input) = match input.as_bytes().first() {
        Some(b'+') => (false, &input[1..]),
        Some(b'-') => (true, &input[1..]),
        _ => (false, input),
    };

    let mut digits = input.bytes().take_while(u8::is_ascii_digit).peekable();
    let parsed = if digits.peek().is_none() {
        default
    } else {
        let number = digits.fold(0u32, |number, digit| {
            number
                .saturating_mul(10)
                .saturating_add(u32::from(digit - b'0'))
                .min(max)
        });
        if negative && number != 0 {
            default
        } else {
            number.clamp(min, max)
        }
    };

    AttrValue::UInt(value.into(), parsed)
}

fn set_clamped_span(
    element: &Element,
    cx: &mut JSContext,
    name: &LocalName,
    value: u32,
    min: u32,
    max: u32,
    default: u32,
) {
    let value = if value > UNSIGNED_LONG_MAX {
        default
    } else {
        value
    };
    element.set_attribute(
        cx,
        name,
        parse_clamped_span(value.to_string(), min, max, default),
    );
}
