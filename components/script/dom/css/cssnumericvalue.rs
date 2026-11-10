/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use cssparser::{Parser, Token};
use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::codegen::GenericBindings::CSSNumericValueBinding::CSSNumericType;
use style::typed_om::numeric_type::NumericType;
use style::values::generics::calc::CalcType;

use super::cssstylevalue::CSSStyleValue;
use crate::dom::bindings::codegen::Bindings::CSSNumericValueBinding::CSSNumericValueMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::USVString;
use crate::dom::css::cssunitvalue::CSSUnitValue;
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSNumericValue {
    css_style_value: CSSStyleValue,
}

impl CSSNumericValue {
    pub(crate) fn new_inherited() -> Self {
        Self {
            css_style_value: CSSStyleValue::new_inherited(String::new()),
        }
    }
}

impl CSSNumericValueMethods<crate::DomTypeHolder> for CSSNumericValue {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-type>
    fn Type(&self) -> CSSNumericType {
        let unit = self
            .downcast::<CSSUnitValue>()
            .expect("CSSUnitValue is the only implemented CSSNumericValue subclass")
            .unit_str();
        let mut numeric_type = CSSNumericType {
            length: None,
            angle: None,
            time: None,
            frequency: None,
            resolution: None,
            flex: None,
            percent: None,
            percentHint: None,
        };
        match NumericType::from_unit_unchecked(unit).as_calc_type() {
            Ok(CalcType::Length) => numeric_type.length = Some(1),
            Ok(CalcType::Angle) => numeric_type.angle = Some(1),
            Ok(CalcType::Time) => numeric_type.time = Some(1),
            Ok(CalcType::Resolution) => numeric_type.resolution = Some(1),
            Ok(CalcType::Percentage) => numeric_type.percent = Some(1),
            Ok(CalcType::Number) => {},
            Err(_) if unit == "fr" => numeric_type.flex = Some(1),
            Err(_) => numeric_type.frequency = Some(1),
        }
        numeric_type
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-to>
    fn To(&self, cx: &mut JSContext, unit: USVString) -> Fallible<DomRoot<CSSUnitValue>> {
        let unit = unit.to_string().to_ascii_lowercase();
        NumericType::try_from_unit(&unit)
            .map_err(|_| Error::Syntax(Some("Unknown CSS unit".into())))?;
        let source = self
            .downcast::<CSSUnitValue>()
            .expect("CSSUnitValue is the only implemented CSSNumericValue subclass");
        let value = if source.unit_str() == unit {
            source.numeric_value()
        } else if let (Some((from_type, from_factor)), Some((to_type, to_factor))) =
            (canonical_factor(source.unit_str()), canonical_factor(&unit))
        {
            if from_type != to_type {
                return Err(Error::Type(c"Incompatible CSS units".to_owned()));
            }
            source.numeric_value() * from_factor / to_factor
        } else {
            return Err(Error::Type(c"Incompatible CSS units".to_owned()));
        };
        if !value.is_finite() {
            return Err(Error::Type(c"CSS unit conversion overflowed".to_owned()));
        }
        Ok(CSSUnitValue::new(cx, &self.global(), value, &unit))
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-parse>
    fn Parse(
        cx: &mut JSContext,
        global: &GlobalScope,
        css_text: USVString,
    ) -> Fallible<DomRoot<CSSNumericValue>> {
        let css_text = css_text.to_string();
        let mut input = Parser::new(&css_text);
        input.skip_whitespace();
        let start = input.position();
        let token = input.next().ok().cloned();
        let token_text = input.slice_from(start).to_owned();
        let unit = match token {
            Some(Token::Number { .. }) => "number".to_owned(),
            Some(Token::Percentage { .. }) => "percent".to_owned(),
            Some(Token::Dimension { unit, .. }) => unit.to_ascii_lowercase(),
            _ => return Err(Error::Syntax(Some("Invalid CSS numeric value".into()))),
        };
        let value = parse_numeric_prefix(&token_text)
            .ok_or_else(|| Error::Syntax(Some("Invalid CSS numeric value".into())))?;
        if input.next().is_ok() || !value.is_finite() || NumericType::try_from_unit(&unit).is_err()
        {
            return Err(Error::Syntax(Some("Invalid CSS numeric value".into())));
        }
        Ok(DomRoot::upcast(CSSUnitValue::new(cx, global, value, &unit)))
    }
}

/// Read the number from the original token, before cssparser narrows it to f32.
fn parse_numeric_prefix(text: &str) -> Option<f64> {
    let bytes = text.as_bytes();
    let mut end = 0;
    if matches!(bytes.first().copied(), Some(b'+' | b'-')) {
        end += 1;
    }
    let integer_start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let has_integer = end != integer_start;
    if bytes.get(end) == Some(&b'.') && bytes.get(end + 1).is_some_and(u8::is_ascii_digit) {
        end += 1;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
    } else if !has_integer {
        return None;
    }
    if matches!(bytes.get(end).copied(), Some(b'e' | b'E')) {
        let mut exponent_end = end + 1;
        if matches!(bytes.get(exponent_end).copied(), Some(b'+' | b'-')) {
            exponent_end += 1;
        }
        let exponent_start = exponent_end;
        while bytes.get(exponent_end).is_some_and(u8::is_ascii_digit) {
            exponent_end += 1;
        }
        if exponent_end != exponent_start {
            end = exponent_end;
        }
    }
    text.get(..end)?.parse().ok()
}

/// Factors relative to the canonical unit within each compatible unit group.
fn canonical_factor(unit: &str) -> Option<(u8, f64)> {
    Some(match unit {
        "px" => (0, 1.0),
        "cm" => (0, 96.0 / 2.54),
        "mm" => (0, 96.0 / 25.4),
        "q" => (0, 96.0 / 101.6),
        "in" => (0, 96.0),
        "pt" => (0, 96.0 / 72.0),
        "pc" => (0, 16.0),
        "deg" => (1, 1.0),
        "grad" => (1, 0.9),
        "rad" => (1, 180.0 / std::f64::consts::PI),
        "turn" => (1, 360.0),
        "s" => (2, 1.0),
        "ms" => (2, 0.001),
        "hz" => (3, 1.0),
        "khz" => (3, 1000.0),
        "dppx" => (4, 1.0),
        "dpi" => (4, 1.0 / 96.0),
        "dpcm" => (4, 2.54 / 96.0),
        _ => return None,
    })
}
