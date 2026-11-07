/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Parsing, serialization and unit conversion for the basic data types of the SVG DOM.
//!
//! <https://svgwg.org/svg2-draft/types.html>

use js::context::JSContext;
use malloc_size_of_derive::MallocSizeOf;

pub(crate) const SVG_LENGTHTYPE_NUMBER: u16 = 1;
pub(crate) const SVG_LENGTHTYPE_PERCENTAGE: u16 = 2;
pub(crate) const SVG_LENGTHTYPE_EMS: u16 = 3;
pub(crate) const SVG_LENGTHTYPE_EXS: u16 = 4;
pub(crate) const SVG_LENGTHTYPE_PX: u16 = 5;
pub(crate) const SVG_LENGTHTYPE_CM: u16 = 6;
pub(crate) const SVG_LENGTHTYPE_MM: u16 = 7;
pub(crate) const SVG_LENGTHTYPE_IN: u16 = 8;
pub(crate) const SVG_LENGTHTYPE_PT: u16 = 9;
pub(crate) const SVG_LENGTHTYPE_PC: u16 = 10;

/// CSS units that SVG lengths accept but that have no unit type in the SVG DOM, whose
/// `unitType` is `SVG_LENGTHTYPE_UNKNOWN`. These codes are only used internally.
const LENGTH_UNIT_Q: u16 = 11;
const LENGTH_UNIT_REM: u16 = 12;
const LENGTH_UNIT_VW: u16 = 13;
const LENGTH_UNIT_VH: u16 = 14;
const LENGTH_UNIT_VMIN: u16 = 15;
const LENGTH_UNIT_VMAX: u16 = 16;

/// The unit suffixes of the length units, indexed by unit code.
const LENGTH_UNITS: [&str; 17] = [
    "", "", "%", "em", "ex", "px", "cm", "mm", "in", "pt", "pc", "q", "rem", "vw", "vh", "vmin",
    "vmax",
];

/// The direction in which a length is measured, which decides what a percentage is a
/// percentage of.
///
/// <https://svgwg.org/svg2-draft/coords.html#Units>
#[derive(Clone, Copy, Debug, JSTraceable, MallocSizeOf, PartialEq)]
pub(crate) enum SVGLengthDirection {
    Horizontal,
    Vertical,
    Other,
}

/// A length in the units it was specified in.
#[derive(Clone, Copy, Debug, JSTraceable, MallocSizeOf, PartialEq)]
pub(crate) struct SVGLengthValue {
    pub(crate) value: f32,
    pub(crate) unit_type: u16,
}

impl SVGLengthValue {
    pub(crate) const fn new(value: f32, unit_type: u16) -> Self {
        Self { value, unit_type }
    }

    pub(crate) const fn number(value: f32) -> Self {
        Self::new(value, SVG_LENGTHTYPE_NUMBER)
    }

    pub(crate) const fn percentage(value: f32) -> Self {
        Self::new(value, SVG_LENGTHTYPE_PERCENTAGE)
    }

    /// Parse a length, a `<length-percentage> | <number>`, with surrounding whitespace.
    pub(crate) fn parse(input: &str) -> Option<Self> {
        let input = input.trim_matches(is_svg_whitespace);
        let (value, rest) = parse_number_prefix(input)?;
        let unit_type = if rest.is_empty() {
            SVG_LENGTHTYPE_NUMBER
        } else if rest == "%" {
            SVG_LENGTHTYPE_PERCENTAGE
        } else {
            let index = LENGTH_UNITS
                .iter()
                .skip(SVG_LENGTHTYPE_EMS as usize)
                .position(|unit| unit.eq_ignore_ascii_case(rest))?;
            index as u16 + SVG_LENGTHTYPE_EMS
        };
        Some(Self { value, unit_type })
    }

    /// The unit type of this length in the SVG DOM, which is unknown for CSS units that the
    /// SVG DOM has no unit type for.
    pub(crate) fn dom_unit_type(&self) -> u16 {
        if Self::is_valid_unit_type(self.unit_type) {
            self.unit_type
        } else {
            0
        }
    }

    /// Whether `unit_type` is one of the length unit types other than unknown.
    pub(crate) fn is_valid_unit_type(unit_type: u16) -> bool {
        (SVG_LENGTHTYPE_NUMBER..=SVG_LENGTHTYPE_PC).contains(&unit_type)
    }

    /// The number of user units in one of `unit_type`, given the context of the length, or
    /// `None` when that context is needed but missing.
    pub(crate) fn user_units_per_unit(
        cx: &mut JSContext,
        unit_type: u16,
        context: &dyn SVGLengthContext,
    ) -> Option<f32> {
        Some(match unit_type {
            SVG_LENGTHTYPE_NUMBER | SVG_LENGTHTYPE_PX => 1.0,
            SVG_LENGTHTYPE_CM => 96.0 / 2.54,
            SVG_LENGTHTYPE_MM => 96.0 / 25.4,
            SVG_LENGTHTYPE_IN => 96.0,
            SVG_LENGTHTYPE_PT => 4.0 / 3.0,
            SVG_LENGTHTYPE_PC => 16.0,
            SVG_LENGTHTYPE_EMS => context.font_size(cx)?,
            SVG_LENGTHTYPE_EXS => context.x_height(cx)?,
            SVG_LENGTHTYPE_PERCENTAGE => context.viewport_length(cx)? / 100.0,
            LENGTH_UNIT_Q => 96.0 / 101.6,
            LENGTH_UNIT_REM => context.root_font_size(cx)?,
            LENGTH_UNIT_VW => context.window_viewport_size(cx)?.0 / 100.0,
            LENGTH_UNIT_VH => context.window_viewport_size(cx)?.1 / 100.0,
            LENGTH_UNIT_VMIN => {
                let (width, height) = context.window_viewport_size(cx)?;
                width.min(height) / 100.0
            },
            LENGTH_UNIT_VMAX => {
                let (width, height) = context.window_viewport_size(cx)?;
                width.max(height) / 100.0
            },
            _ => return None,
        })
    }

    /// This length in user units.
    pub(crate) fn to_user_units(
        self,
        cx: &mut JSContext,
        context: &dyn SVGLengthContext,
    ) -> Option<f32> {
        Some(self.value * Self::user_units_per_unit(cx, self.unit_type, context)?)
    }

    /// The length of `user_units` user units in `unit_type`.
    pub(crate) fn from_user_units(
        cx: &mut JSContext,
        user_units: f32,
        unit_type: u16,
        context: &dyn SVGLengthContext,
    ) -> Option<Self> {
        let factor = Self::user_units_per_unit(cx, unit_type, context)?;
        if factor == 0.0 {
            return None;
        }
        // Divide in double precision, so that, for example, 120 user units in a viewport
        // of 400 are exactly 30%.
        let value = if unit_type == SVG_LENGTHTYPE_PERCENTAGE {
            let viewport = context.viewport_length(cx)? as f64;
            user_units as f64 / viewport * 100.0
        } else {
            user_units as f64 / factor as f64
        };
        Some(Self::new(value as f32, unit_type))
    }
}

impl std::fmt::Display for SVGLengthValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_number(f, self.value)?;
        f.write_str(LENGTH_UNITS.get(self.unit_type as usize).unwrap_or(&""))
    }
}

/// What is needed to convert relative lengths to user units.
pub(crate) trait SVGLengthContext {
    /// The computed font size, for `em`.
    fn font_size(&self, cx: &mut JSContext) -> Option<f32>;
    /// The x-height of the first available font, for `ex`.
    fn x_height(&self, cx: &mut JSContext) -> Option<f32>;
    /// The size of the viewport in the direction of the length, for percentages.
    fn viewport_length(&self, cx: &mut JSContext) -> Option<f32>;
    /// The computed font size of the root element, for `rem`.
    fn root_font_size(&self, cx: &mut JSContext) -> Option<f32>;
    /// The size of the viewport of the window, for the viewport-percentage units.
    fn window_viewport_size(&self, cx: &mut JSContext) -> Option<(f32, f32)>;
}

/// The context of a length that has no element, like one from `createSVGLength()`.
///
/// > If the SVGLength object has no associated element, the font-size basis is the
/// > initial value of the font-size property, and percentages are converted as if the
/// > viewport were 100 user units in size.
pub(crate) struct NoLengthContext;

/// The initial value of `font-size`, `medium`, in CSS pixels.
pub(crate) const INITIAL_FONT_SIZE: f32 = 16.0;

impl SVGLengthContext for NoLengthContext {
    fn font_size(&self, _: &mut JSContext) -> Option<f32> {
        Some(INITIAL_FONT_SIZE)
    }
    fn x_height(&self, _: &mut JSContext) -> Option<f32> {
        Some(INITIAL_FONT_SIZE / 2.0)
    }
    fn viewport_length(&self, _: &mut JSContext) -> Option<f32> {
        Some(100.0)
    }
    fn root_font_size(&self, _: &mut JSContext) -> Option<f32> {
        Some(INITIAL_FONT_SIZE)
    }
    fn window_viewport_size(&self, _: &mut JSContext) -> Option<(f32, f32)> {
        None
    }
}

/// <https://www.w3.org/TR/SVG11/types.html#BasicDataTypes>: whitespace is space, tab,
/// carriage return and line feed.
pub(crate) fn is_svg_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n' | '\x0C')
}

/// Parse a `<number>` at the start of `input`, returning it and the rest of the input.
///
/// <https://www.w3.org/TR/css-syntax-3/#consume-number>
pub(crate) fn parse_number_prefix(input: &str) -> Option<(f32, &str)> {
    let bytes = input.as_bytes();
    let mut position = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        position += 1;
    }
    let integer_start = position;
    while bytes.get(position).is_some_and(u8::is_ascii_digit) {
        position += 1;
    }
    let mut has_digits = position > integer_start;
    if bytes.get(position) == Some(&b'.') && bytes.get(position + 1).is_some_and(u8::is_ascii_digit)
    {
        position += 1;
        while bytes.get(position).is_some_and(u8::is_ascii_digit) {
            position += 1;
        }
        has_digits = true;
    }
    if !has_digits {
        return None;
    }
    if matches!(bytes.get(position), Some(b'e' | b'E')) {
        let mut exponent = position + 1;
        if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
            exponent += 1;
        }
        if bytes.get(exponent).is_some_and(u8::is_ascii_digit) {
            position = exponent;
            while bytes.get(position).is_some_and(u8::is_ascii_digit) {
                position += 1;
            }
        }
    }
    let value: f32 = input[..position].parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    Some((value, &input[position..]))
}

/// Parse a `<number>` with surrounding whitespace.
pub(crate) fn parse_number(input: &str) -> Option<f32> {
    let input = input.trim_matches(is_svg_whitespace);
    match parse_number_prefix(input)? {
        (value, "") => Some(value),
        _ => None,
    }
}

/// Parse an `<integer>` with surrounding whitespace.
pub(crate) fn parse_integer(input: &str) -> Option<i32> {
    let input = input.trim_matches(is_svg_whitespace);
    let digits = input.strip_prefix('+').unwrap_or(input);
    if digits.is_empty() ||
        !digits
            .strip_prefix('-')
            .unwrap_or(digits)
            .bytes()
            .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    digits.parse().ok()
}

/// Split a list of values separated by whitespace and optionally a comma.
pub(crate) fn split_list(input: &str) -> Option<Vec<&str>> {
    let mut items = Vec::new();
    let mut rest = input.trim_matches(is_svg_whitespace);
    if rest.is_empty() {
        return Some(items);
    }
    loop {
        let end = rest
            .find(|c: char| is_svg_whitespace(c) || c == ',')
            .unwrap_or(rest.len());
        let item = &rest[..end];
        if item.is_empty() {
            return None;
        }
        items.push(item);
        rest = rest[end..].trim_start_matches(is_svg_whitespace);
        if let Some(after_comma) = rest.strip_prefix(',') {
            rest = after_comma.trim_start_matches(is_svg_whitespace);
            if rest.is_empty() {
                return None;
            }
        }
        if rest.is_empty() {
            return Some(items);
        }
    }
}

/// Parse a `<number-optional-number>`, returning both numbers.
pub(crate) fn parse_number_optional_number(input: &str) -> Option<(f32, f32)> {
    match split_list(input)?.as_slice() {
        [first] => {
            let first = parse_number(first)?;
            Some((first, first))
        },
        [first, second] => Some((parse_number(first)?, parse_number(second)?)),
        _ => None,
    }
}

/// Parse an `<integer> <integer>?`, returning both integers.
pub(crate) fn parse_integer_optional_integer(input: &str) -> Option<(i32, i32)> {
    match split_list(input)?.as_slice() {
        [first] => {
            let first = parse_integer(first)?;
            Some((first, first))
        },
        [first, second] => Some((parse_integer(first)?, parse_integer(second)?)),
        _ => None,
    }
}

/// Write `value` the way browsers serialize the numbers of the SVG DOM: the shortest
/// representation that converts back to the same single-precision number.
pub(crate) fn write_number(f: &mut impl std::fmt::Write, value: f32) -> std::fmt::Result {
    if value == 0.0 {
        return f.write_str("0");
    }
    write!(f, "{}", value)
}

/// Serialize a number of the SVG DOM.
pub(crate) fn number_to_string(value: f32) -> String {
    let mut string = String::new();
    let _ = write_number(&mut string, value);
    string
}

/// Define the getter of an IDL attribute that returns an `SVGAnimatedLength`, which
/// reflects the content attribute with the same name.
macro_rules! svg_animated_length {
    ($method:ident, $name:literal, $direction:ident, $initial:expr) => {
        fn $method(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<
            crate::dom::svg::svganimatedlength::SVGAnimatedLength,
        > {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .animated_length(
                    cx,
                    $name,
                    html5ever::LocalName::from($name),
                    crate::dom::svg::svgtypes::SVGLengthDirection::$direction,
                    $initial,
                )
        }
    };
}
pub(crate) use svg_animated_length;

/// Define the getter of an IDL attribute that returns an `SVGAnimatedEnumeration`.
macro_rules! svg_animated_enumeration {
    ($method:ident, $name:literal, $attribute:literal, $keywords:expr, $initial:expr) => {
        fn $method(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<
            crate::dom::svg::svganimatedenumeration::SVGAnimatedEnumeration,
        > {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .animated_enumeration(
                    cx,
                    $name,
                    html5ever::LocalName::from($attribute),
                    $keywords,
                    $initial,
                )
        }
    };
}
pub(crate) use svg_animated_enumeration;

/// Define the getter of an IDL attribute that returns an `SVGAnimatedNumber`.
macro_rules! svg_animated_number {
    ($method:ident, $name:literal, $attribute:literal, $part:ident, $initial:expr) => {
        fn $method(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<
            crate::dom::svg::svganimatednumber::SVGAnimatedNumber,
        > {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .animated_number(
                    cx,
                    $name,
                    html5ever::LocalName::from($attribute),
                    crate::dom::svg::svganimatednumber::SVGNumberPart::$part,
                    $initial,
                )
        }
    };
}
pub(crate) use svg_animated_number;

/// Define the getter of an IDL attribute that returns an `SVGAnimatedString`.
macro_rules! svg_animated_string {
    ($method:ident, $name:literal, $attribute:literal) => {
        fn $method(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<
            crate::dom::svg::svganimatedstring::SVGAnimatedString,
        > {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .animated_string(cx, $name, html5ever::LocalName::from($attribute))
        }
    };
}
pub(crate) use svg_animated_string;

/// Define the `href` getter of the `SVGURIReference` mixin.
macro_rules! svg_uri_reference {
    () => {
        fn Href(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<
            crate::dom::svg::svganimatedstring::SVGAnimatedString,
        > {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .href(cx)
        }
    };
}
pub(crate) use svg_uri_reference;

/// Define the getter of an IDL attribute that returns an `SVGAnimatedInteger`.
macro_rules! svg_animated_integer {
    ($method:ident, $name:literal, $attribute:literal, $part:ident, $initial:expr) => {
        fn $method(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<
            crate::dom::svg::svganimatedinteger::SVGAnimatedInteger,
        > {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .animated_integer(
                    cx,
                    $name,
                    html5ever::LocalName::from($attribute),
                    crate::dom::svg::svganimatednumber::SVGNumberPart::$part,
                    $initial,
                )
        }
    };
}
pub(crate) use svg_animated_integer;

/// Define the getter of an IDL attribute that returns an `SVGAnimatedBoolean`.
macro_rules! svg_animated_boolean {
    ($method:ident, $name:literal, $attribute:literal) => {
        fn $method(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<
            crate::dom::svg::svganimatedboolean::SVGAnimatedBoolean,
        > {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .animated_boolean(cx, $name, html5ever::LocalName::from($attribute))
        }
    };
}
pub(crate) use svg_animated_boolean;

/// Define the members of the `SVGFilterPrimitiveStandardAttributes` mixin.
///
/// <https://drafts.fxtf.org/filter-effects/#InterfaceSVGFilterPrimitiveStandardAttributes>
macro_rules! svg_filter_primitive_standard_attributes {
    () => {
        crate::dom::svg::svgtypes::svg_animated_length!(
            X,
            "x",
            Horizontal,
            crate::dom::svg::svgtypes::SVGLengthValue::percentage(0.0)
        );
        crate::dom::svg::svgtypes::svg_animated_length!(
            Y,
            "y",
            Vertical,
            crate::dom::svg::svgtypes::SVGLengthValue::percentage(0.0)
        );
        crate::dom::svg::svgtypes::svg_animated_length!(
            Width,
            "width",
            Horizontal,
            crate::dom::svg::svgtypes::SVGLengthValue::percentage(100.0)
        );
        crate::dom::svg::svgtypes::svg_animated_length!(
            Height,
            "height",
            Vertical,
            crate::dom::svg::svgtypes::SVGLengthValue::percentage(100.0)
        );
        crate::dom::svg::svgtypes::svg_animated_string!(Result, "result", "result");
    };
}
pub(crate) use svg_filter_primitive_standard_attributes;

/// Define `setStdDeviation()`, which sets the `stdDeviation` attribute.
macro_rules! svg_set_std_deviation {
    () => {
        fn SetStdDeviation(
            &self,
            cx: &mut js::context::JSContext,
            x: crate::dom::bindings::num::Finite<f32>,
            y: crate::dom::bindings::num::Finite<f32>,
        ) {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .set_svg_attribute(
                    cx,
                    &html5ever::LocalName::from("stdDeviation"),
                    format!(
                        "{} {}",
                        crate::dom::svg::svgtypes::number_to_string(*x),
                        crate::dom::svg::svgtypes::number_to_string(*y)
                    ),
                );
        }
    };
}
pub(crate) use svg_set_std_deviation;

/// Define the `preserveAspectRatio` getter.
macro_rules! svg_preserve_aspect_ratio {
    () => {
        fn PreserveAspectRatio(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<
            crate::dom::svg::svganimatedpreserveaspectratio::SVGAnimatedPreserveAspectRatio,
        > {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .preserve_aspect_ratio(cx)
        }
    };
}
pub(crate) use svg_preserve_aspect_ratio;

/// Define the members of the `SVGFitToViewBox` mixin.
///
/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGFitToViewBox>
macro_rules! svg_fit_to_view_box {
    () => {
        fn ViewBox(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<crate::dom::svg::svganimatedrect::SVGAnimatedRect>
        {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .view_box(cx)
        }

        crate::dom::svg::svgtypes::svg_preserve_aspect_ratio!();
    };
}
pub(crate) use svg_fit_to_view_box;

/// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAngle__SVG_ANGLETYPE_UNSPECIFIED>
pub(crate) const SVG_ANGLETYPE_UNSPECIFIED: u16 = 1;
pub(crate) const SVG_ANGLETYPE_GRAD: u16 = 4;
/// `turn`, which has no unit type in the SVG DOM. This code is only used internally.
const ANGLE_UNIT_TURN: u16 = 5;

/// The unit suffixes of the angle units, indexed by unit code.
const ANGLE_UNITS: [&str; 6] = ["", "", "deg", "rad", "grad", "turn"];

/// An angle in the units it was specified in.
#[derive(Clone, Copy, Debug, JSTraceable, MallocSizeOf, PartialEq)]
pub(crate) struct SVGAngleValue {
    pub(crate) value: f32,
    pub(crate) unit_type: u16,
}

impl SVGAngleValue {
    pub(crate) const fn new(value: f32, unit_type: u16) -> Self {
        Self { value, unit_type }
    }

    /// Parse an `<angle> | <number>`, with surrounding whitespace.
    pub(crate) fn parse(input: &str) -> Option<Self> {
        let input = input.trim_matches(is_svg_whitespace);
        let (value, rest) = parse_number_prefix(input)?;
        let unit_type = if rest.is_empty() {
            SVG_ANGLETYPE_UNSPECIFIED
        } else {
            ANGLE_UNITS
                .iter()
                .skip(2)
                .position(|unit| unit.eq_ignore_ascii_case(rest))? as u16 +
                2
        };
        Some(Self { value, unit_type })
    }

    /// The unit type of this angle in the SVG DOM.
    pub(crate) fn dom_unit_type(&self) -> u16 {
        if Self::is_valid_unit_type(self.unit_type) {
            self.unit_type
        } else {
            0
        }
    }

    /// Whether `unit_type` is one of the angle unit types other than unknown.
    pub(crate) fn is_valid_unit_type(unit_type: u16) -> bool {
        (SVG_ANGLETYPE_UNSPECIFIED..=SVG_ANGLETYPE_GRAD).contains(&unit_type)
    }

    /// The number of degrees in one of `unit_type`.
    fn degrees_per_unit(unit_type: u16) -> f32 {
        match unit_type {
            3 => 180.0 / std::f32::consts::PI,
            SVG_ANGLETYPE_GRAD => 0.9,
            ANGLE_UNIT_TURN => 360.0,
            _ => 1.0,
        }
    }

    /// This angle in degrees.
    pub(crate) fn to_degrees(self) -> f32 {
        self.value * Self::degrees_per_unit(self.unit_type)
    }

    /// The angle of `degrees` degrees in `unit_type`.
    pub(crate) fn from_degrees(degrees: f32, unit_type: u16) -> Self {
        Self::new(degrees / Self::degrees_per_unit(unit_type), unit_type)
    }
}

impl std::fmt::Display for SVGAngleValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_number(f, self.value)?;
        f.write_str(ANGLE_UNITS.get(self.unit_type as usize).unwrap_or(&""))
    }
}

/// Define the getter of an IDL attribute that returns an `SVGAnimatedNumberList`.
macro_rules! svg_animated_number_list {
    ($method:ident, $name:literal, $attribute:literal) => {
        fn $method(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<
            crate::dom::svg::svganimatednumberlist::SVGAnimatedNumberList,
        > {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .animated_number_list(cx, $name, html5ever::LocalName::from($attribute))
        }
    };
}
pub(crate) use svg_animated_number_list;

/// Define the getter of an IDL attribute that returns an `SVGAnimatedLengthList`.
macro_rules! svg_animated_length_list {
    ($method:ident, $name:literal, $direction:ident) => {
        fn $method(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<
            crate::dom::svg::svganimatedlengthlist::SVGAnimatedLengthList,
        > {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .animated_length_list(
                    cx,
                    $name,
                    html5ever::LocalName::from($name),
                    crate::dom::svg::svgtypes::SVGLengthDirection::$direction,
                )
        }
    };
}
pub(crate) use svg_animated_length_list;

/// Define the members of the `SVGAnimatedPoints` mixin.
///
/// <https://svgwg.org/svg2-draft/shapes.html#InterfaceSVGAnimatedPoints>
macro_rules! svg_animated_points {
    () => {
        fn Points(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<crate::dom::svg::svgpointlist::SVGPointList> {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .point_list(cx, false)
        }

        fn AnimatedPoints(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<crate::dom::svg::svgpointlist::SVGPointList> {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .point_list(cx, true)
        }
    };
}
pub(crate) use svg_animated_points;

/// Define the getter of an IDL attribute that returns an `SVGAnimatedTransformList`.
macro_rules! svg_animated_transform_list {
    ($method:ident, $name:literal, $attribute:literal) => {
        fn $method(
            &self,
            cx: &mut js::context::JSContext,
        ) -> crate::dom::bindings::root::DomRoot<
            crate::dom::svg::svganimatedtransformlist::SVGAnimatedTransformList,
        > {
            use crate::dom::bindings::inheritance::Castable;
            self.upcast::<crate::dom::svg::svgelement::SVGElement>()
                .animated_transform_list(cx, $name, html5ever::LocalName::from($attribute))
        }
    };
}
pub(crate) use svg_animated_transform_list;
