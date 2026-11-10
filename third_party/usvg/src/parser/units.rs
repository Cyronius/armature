// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use svgtypes::{Length, LengthUnit as Unit};

use super::converter;
use super::svgtree::{AId, SvgNode};
use crate::Units;

// Armature (AU-008): Resolve CSS calc() after the SVG viewport and font size
// are known. Keeping number and length values distinct rejects invalid math.
#[derive(Clone, Copy)]
enum CalcValue {
    Number(f32),
    Length(f32),
}

impl CalcValue {
    fn add(self, other: Self, subtract: bool) -> Option<Self> {
        let sign = if subtract { -1.0 } else { 1.0 };
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => Some(Self::Number(a + sign * b)),
            (Self::Length(a), Self::Length(b)) => Some(Self::Length(a + sign * b)),
            _ => None,
        }
    }

    fn multiply(self, other: Self, divide: bool) -> Option<Self> {
        match (self, other, divide) {
            (Self::Number(a), Self::Number(b), false) => Some(Self::Number(a * b)),
            (Self::Length(a), Self::Number(b), false) |
            (Self::Number(b), Self::Length(a), false) => Some(Self::Length(a * b)),
            (Self::Number(a), Self::Number(b), true) if b != 0.0 => Some(Self::Number(a / b)),
            (Self::Length(a), Self::Number(b), true) if b != 0.0 => Some(Self::Length(a / b)),
            _ => None,
        }
    }

    fn negate(self) -> Self {
        match self {
            Self::Number(value) => Self::Number(-value),
            Self::Length(value) => Self::Length(-value),
        }
    }

    fn value(self) -> f32 {
        match self {
            Self::Number(value) | Self::Length(value) => value,
        }
    }
}

struct CalcParser<'a> {
    text: &'a str,
    offset: usize,
    convert: &'a dyn Fn(Length) -> f32,
}

impl CalcParser<'_> {
    fn skip_whitespace(&mut self) {
        while self.text.as_bytes().get(self.offset).is_some_and(u8::is_ascii_whitespace) {
            self.offset += 1;
        }
    }

    fn consume(&mut self, character: u8) -> bool {
        let before_whitespace = self.offset;
        self.skip_whitespace();
        if self.text.as_bytes().get(self.offset) == Some(&character) {
            self.offset += 1;
            true
        } else {
            self.offset = before_whitespace;
            false
        }
    }

    fn sum(&mut self, depth: usize) -> Option<CalcValue> {
        let mut value = self.product(depth)?;
        loop {
            let before_whitespace = self.offset;
            self.skip_whitespace();
            let operator = self.text.as_bytes().get(self.offset).copied();
            if !matches!(operator, Some(b'+' | b'-')) {
                self.offset = before_whitespace;
                break;
            }
            // CSS requires whitespace on both sides of binary + and -.
            if self.offset == before_whitespace ||
                !self
                    .text
                    .as_bytes()
                    .get(self.offset + 1)
                    .is_some_and(u8::is_ascii_whitespace)
            {
                return None;
            }
            self.offset += 1;
            value = value.add(self.product(depth)?, operator == Some(b'-'))?;
        }
        Some(value)
    }

    fn product(&mut self, depth: usize) -> Option<CalcValue> {
        let mut value = self.primary(depth)?;
        loop {
            let divide = if self.consume(b'*') {
                false
            } else if self.consume(b'/') {
                true
            } else {
                break;
            };
            value = value.multiply(self.primary(depth)?, divide)?;
        }
        Some(value)
    }

    fn primary(&mut self, depth: usize) -> Option<CalcValue> {
        if depth >= 64 {
            return None;
        }
        self.skip_whitespace();
        if self.consume(b'+') {
            return self.primary(depth + 1);
        }
        if self.consume(b'-') {
            return self.primary(depth + 1).map(CalcValue::negate);
        }
        if self.consume(b'(') {
            let value = self.sum(depth + 1)?;
            return self.consume(b')').then_some(value);
        }
        if self
            .text
            .get(self.offset..self.offset + 5)
            .is_some_and(|function| function.eq_ignore_ascii_case("calc("))
        {
            self.offset += 5;
            let value = self.sum(depth + 1)?;
            return self.consume(b')').then_some(value);
        }

        let bytes = self.text.as_bytes();
        let start = self.offset;
        while bytes.get(self.offset).is_some_and(u8::is_ascii_digit) {
            self.offset += 1;
        }
        let mut has_digits = self.offset != start;
        if bytes.get(self.offset) == Some(&b'.') {
            self.offset += 1;
            let fractional_start = self.offset;
            while bytes.get(self.offset).is_some_and(u8::is_ascii_digit) {
                self.offset += 1;
            }
            has_digits |= self.offset != fractional_start;
        }
        if !has_digits {
            return None;
        }
        if matches!(bytes.get(self.offset), Some(b'e' | b'E')) {
            let exponent_start = self.offset;
            self.offset += 1;
            if matches!(bytes.get(self.offset), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            let digits_start = self.offset;
            while bytes.get(self.offset).is_some_and(u8::is_ascii_digit) {
                self.offset += 1;
            }
            if self.offset == digits_start {
                self.offset = exponent_start;
            }
        }
        let number_end = self.offset;
        while bytes
            .get(self.offset)
            .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'%')
        {
            self.offset += 1;
        }
        if number_end == self.offset {
            return self.text[start..number_end]
                .parse::<f32>()
                .ok()
                .map(CalcValue::Number);
        }
        let length = self.text[start..self.offset].parse::<Length>().ok()?;
        Some(CalcValue::Length((self.convert)(length)))
    }
}

pub(crate) fn convert_calc_length(
    text: &str,
    node: SvgNode,
    aid: AId,
    object_units: Units,
    state: &converter::State,
) -> Option<f32> {
    let convert = |length| convert_length(length, node, aid, object_units, state);
    parse_calc_length(text, &convert)
}

fn parse_calc_length(text: &str, convert: &dyn Fn(Length) -> f32) -> Option<f32> {
    let mut parser = CalcParser {
        text: text.trim(),
        offset: 0,
        convert,
    };
    if !parser
        .text
        .get(..5)
        .is_some_and(|function| function.eq_ignore_ascii_case("calc("))
    {
        return None;
    }
    parser.offset = 5;
    let value = parser.sum(0)?;
    if !parser.consume(b')') {
        return None;
    }
    parser.skip_whitespace();
    (parser.offset == parser.text.len() && value.value().is_finite()).then_some(value.value())
}

#[cfg(test)]
mod calc_tests {
    use super::{Length, Unit, parse_calc_length};

    #[test]
    fn arithmetic_and_types() {
        let convert = |length: Length| match length.unit {
            Unit::None | Unit::Px => length.number as f32,
            Unit::Percent => length.number as f32 * 2.0,
            Unit::Em => length.number as f32 * 40.0,
            _ => panic!("unexpected unit"),
        };
        assert_eq!(parse_calc_length("calc(80px + 60% - 2em)", &convert), Some(120.0));
        assert_eq!(parse_calc_length("calc(5 * 5%)", &convert), Some(50.0));
        assert_eq!(parse_calc_length("calc(3 * (10px + 2px))", &convert), Some(36.0));
        assert_eq!(parse_calc_length("calc(10px / 2)", &convert), Some(5.0));
        assert_eq!(parse_calc_length("calc(10px + 2)", &convert), None);
        assert_eq!(parse_calc_length("calc(10px * 10px)", &convert), None);
        assert_eq!(parse_calc_length("calc(10px+2px)", &convert), None);
        assert_eq!(parse_calc_length("calc(10px / 0)", &convert), None);
        let nested = format!("calc({}1px{})", "(".repeat(65), ")".repeat(65));
        assert_eq!(parse_calc_length(&nested, &convert), None);
    }
}

#[inline(never)]
pub(crate) fn convert_length(
    length: Length,
    node: SvgNode,
    aid: AId,
    object_units: Units,
    state: &converter::State,
) -> f32 {
    let dpi = state.opt.dpi;
    let n = length.number as f32;
    match length.unit {
        Unit::None | Unit::Px => n,
        Unit::Em => n * resolve_font_size(node, state),
        Unit::Ex => n * resolve_font_size(node, state) / 2.0,
        Unit::In => n * dpi,
        Unit::Cm => n * dpi / 2.54,
        Unit::Mm => n * dpi / 25.4,
        Unit::Pt => n * dpi / 72.0,
        Unit::Pc => n * dpi / 6.0,
        Unit::Percent => {
            if object_units == Units::ObjectBoundingBox {
                n / 100.0
            } else {
                let view_box = state.view_box;

                match aid {
                    AId::Cx
                    | AId::Dx
                    | AId::Fx
                    | AId::MarkerWidth
                    | AId::RefX
                    | AId::Rx
                    | AId::Width
                    | AId::X
                    | AId::X1
                    | AId::X2 => convert_percent(length, view_box.width()),
                    AId::Cy
                    | AId::Dy
                    | AId::Fy
                    | AId::Height
                    | AId::MarkerHeight
                    | AId::RefY
                    | AId::Ry
                    | AId::Y
                    | AId::Y1
                    | AId::Y2 => convert_percent(length, view_box.height()),
                    _ => {
                        let mut vb_len = view_box.width().powi(2) + view_box.height().powi(2);
                        vb_len = (vb_len / 2.0).sqrt();
                        convert_percent(length, vb_len)
                    }
                }
            }
        }
    }
}

pub(crate) fn convert_user_length(
    length: Length,
    node: SvgNode,
    aid: AId,
    state: &converter::State,
) -> f32 {
    convert_length(length, node, aid, Units::UserSpaceOnUse, state)
}

#[inline(never)]
pub(crate) fn convert_list(node: SvgNode, aid: AId, state: &converter::State) -> Option<Vec<f32>> {
    if let Some(text) = node.attribute::<&str>(aid) {
        let mut num_list = Vec::new();
        for length in svgtypes::LengthListParser::from(text).flatten() {
            num_list.push(convert_user_length(length, node, aid, state));
        }

        Some(num_list)
    } else {
        None
    }
}

fn convert_percent(length: Length, base: f32) -> f32 {
    base * (length.number as f32) / 100.0
}

#[inline(never)]
pub(crate) fn resolve_font_size(node: SvgNode, state: &converter::State) -> f32 {
    let nodes: Vec<_> = node.ancestors().collect();
    let mut font_size = state.opt.font_size;
    for n in nodes.iter().rev().skip(1) {
        // skip Root
        if let Some(length) = n.try_attribute::<Length>(AId::FontSize) {
            let dpi = state.opt.dpi;
            let n = length.number as f32;
            font_size = match length.unit {
                Unit::None | Unit::Px => n,
                Unit::Em => n * font_size,
                Unit::Ex => n * font_size / 2.0,
                Unit::In => n * dpi,
                Unit::Cm => n * dpi / 2.54,
                Unit::Mm => n * dpi / 25.4,
                Unit::Pt => n * dpi / 72.0,
                Unit::Pc => n * dpi / 6.0,
                Unit::Percent => {
                    // If `font-size` has percent units that it's value
                    // is relative to the parent node `font-size`.
                    length.number as f32 * font_size * 0.01
                }
            }
        } else if let Some(name) = n.attribute(AId::FontSize) {
            font_size = convert_named_font_size(name, font_size);
        }
    }

    font_size
}

fn convert_named_font_size(name: &str, parent_font_size: f32) -> f32 {
    let factor = match name {
        "xx-small" => -3,
        "x-small" => -2,
        "small" => -1,
        "medium" => 0,
        "large" => 1,
        "x-large" => 2,
        "xx-large" => 3,
        "smaller" => -1,
        "larger" => 1,
        _ => {
            log::warn!("Invalid 'font-size' value: '{}'.", name);
            0
        }
    };

    // 'On a computer screen a scaling factor of 1.2 is suggested between adjacent indexes.'
    parent_font_size * 1.2f32.powi(factor)
}
