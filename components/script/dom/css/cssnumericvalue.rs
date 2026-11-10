/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::BTreeMap;

use cssparser::{ParseError as CssParseError, Parser, Token};
use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::codegen::GenericBindings::CSSNumericValueBinding::{
    CSSNumericBaseType, CSSNumericType,
};
use style::typed_om::numeric_type::{NumericBaseType, NumericType};
use style::values::generics::Optional;

use super::cssmathclamp::CSSMathClamp;
use super::cssmathinvert::CSSMathInvert;
use super::cssmathmax::CSSMathMax;
use super::cssmathmin::CSSMathMin;
use super::cssmathnegate::CSSMathNegate;
use super::cssmathproduct::CSSMathProduct;
use super::cssmathsum::CSSMathSum;
use super::cssmathvalue::CSSMathValue;
use super::cssstylevalue::CSSStyleValue;
use crate::dom::bindings::codegen::Bindings::CSSMathClampBinding::CSSMathClampMethods;
use crate::dom::bindings::codegen::Bindings::CSSMathInvertBinding::CSSMathInvertMethods;
use crate::dom::bindings::codegen::Bindings::CSSMathMaxBinding::CSSMathMaxMethods;
use crate::dom::bindings::codegen::Bindings::CSSMathMinBinding::CSSMathMinMethods;
use crate::dom::bindings::codegen::Bindings::CSSMathNegateBinding::CSSMathNegateMethods;
use crate::dom::bindings::codegen::Bindings::CSSMathProductBinding::CSSMathProductMethods;
use crate::dom::bindings::codegen::Bindings::CSSMathSumBinding::CSSMathSumMethods;
use crate::dom::bindings::codegen::Bindings::CSSNumericValueBinding::CSSNumericValueMethods;
use crate::dom::bindings::codegen::UnionTypes::DoubleOrCSSNumericValue;
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

#[derive(Clone, Copy)]
enum ArithmeticOp {
    Add,
    Sub,
    Mul,
    Div,
    Min,
    Max,
}

#[derive(Clone)]
struct SumItem {
    value: f64,
    units: BTreeMap<String, i32>,
}

impl SumItem {
    fn unit_value(&self) -> Option<(f64, &str)> {
        match self.units.len() {
            0 => Some((self.value, "number")),
            1 => {
                let (unit, exponent) = self.units.first_key_value()?;
                (*exponent == 1).then_some((self.value, unit.as_str()))
            },
            _ => None,
        }
    }
}

impl CSSNumericValue {
    pub(crate) fn new_inherited() -> Self {
        Self {
            css_style_value: CSSStyleValue::new_inherited(String::new()),
        }
    }

    pub(crate) fn numeric_type(&self) -> NumericType {
        if let Some(unit) = self.downcast::<CSSUnitValue>() {
            NumericType::from_unit_unchecked(unit.unit_str())
        } else {
            self.downcast::<CSSMathValue>()
                .expect("Every CSSNumericValue is a unit or math value")
                .numeric_type()
        }
    }

    pub(crate) fn serialize(&self) -> String {
        if let Some(unit) = self.downcast::<CSSUnitValue>() {
            unit.serialize()
        } else {
            self.downcast::<CSSMathValue>()
                .expect("Every CSSNumericValue is a unit or math value")
                .serialize()
        }
    }

    /// Represent the value as a sum of values and unit powers, using canonical units where
    /// absolute units can be converted without a style or layout context.
    fn sum_value(&self) -> Option<Vec<SumItem>> {
        if let Some(unit) = self.downcast::<CSSUnitValue>() {
            let mut value = unit.numeric_value();
            let mut name = unit.unit_str();
            if let Some((group, factor)) = canonical_factor(name) {
                value *= factor;
                name = canonical_unit(group);
            }
            let mut units = BTreeMap::new();
            if name != "number" {
                units.insert(name.to_owned(), 1);
            }
            return Some(vec![SumItem { value, units }]);
        }
        if let Some(sum) = self.downcast::<CSSMathSum>() {
            let mut result: Vec<SumItem> = Vec::new();
            for value in sum.values() {
                for item in value.sum_value()? {
                    if let Some(existing) =
                        result.iter_mut().find(|entry| entry.units == item.units)
                    {
                        existing.value += item.value;
                    } else {
                        result.push(item);
                    }
                }
            }
            return Some(result);
        }
        if let Some(negate) = self.downcast::<CSSMathNegate>() {
            let mut result = negate.value().sum_value()?;
            for item in &mut result {
                item.value = -item.value;
            }
            return Some(result);
        }
        if let Some(product) = self.downcast::<CSSMathProduct>() {
            let mut result = vec![SumItem {
                value: 1.0,
                units: BTreeMap::new(),
            }];
            for value in product.values() {
                let factors = value.sum_value()?;
                let mut next = Vec::new();
                for item in &result {
                    for factor in &factors {
                        let mut units = item.units.clone();
                        for (unit, power) in &factor.units {
                            *units.entry(unit.clone()).or_default() += power;
                        }
                        units.retain(|_, power| *power != 0);
                        next.push(SumItem {
                            value: item.value * factor.value,
                            units,
                        });
                    }
                }
                result = next;
            }
            return Some(result);
        }
        if let Some(invert) = self.downcast::<CSSMathInvert>() {
            let mut result = invert.value().sum_value()?;
            if result.len() != 1 || result[0].value == 0.0 {
                return None;
            }
            result[0].value = 1.0 / result[0].value;
            for power in result[0].units.values_mut() {
                *power = -*power;
            }
            return Some(result);
        }
        let operands = if let Some(min) = self.downcast::<CSSMathMin>() {
            min.values()
        } else if let Some(max) = self.downcast::<CSSMathMax>() {
            max.values()
        } else {
            self.downcast::<crate::dom::css::cssmathclamp::CSSMathClamp>()?
                .values()
                .into_iter()
                .collect()
        };
        let mut items = Vec::new();
        for operand in operands {
            let mut values = operand.sum_value()?;
            if values.len() != 1 {
                return None;
            }
            items.push(values.remove(0));
        }
        let units = &items.first()?.units;
        if items.iter().any(|item| &item.units != units) {
            return None;
        }
        let mut chosen = items[0].clone();
        if self.is::<CSSMathMin>() {
            for item in items.iter().skip(1) {
                chosen.value = chosen.value.min(item.value);
            }
        } else if self.is::<CSSMathMax>() {
            for item in items.iter().skip(1) {
                chosen.value = chosen.value.max(item.value);
            }
        } else {
            chosen.value = items[0].value.max(items[1].value.min(items[2].value));
        }
        Some(vec![chosen])
    }

    fn negate(cx: &mut JSContext, value: DomRoot<Self>) -> DomRoot<Self> {
        if let Some(negate) = value.downcast::<CSSMathNegate>() {
            return negate.value();
        }
        let global = value.global();
        if let Some(unit) = value.downcast::<CSSUnitValue>() {
            return DomRoot::upcast(CSSUnitValue::new(
                cx,
                &global,
                -unit.numeric_value(),
                unit.unit_str(),
            ));
        }
        DomRoot::upcast(CSSMathNegate::Constructor(
            cx,
            &global,
            None,
            DoubleOrCSSNumericValue::CSSNumericValue(value),
        ))
    }

    fn invert(cx: &mut JSContext, value: DomRoot<Self>) -> Fallible<DomRoot<Self>> {
        if let Some(invert) = value.downcast::<CSSMathInvert>() {
            return Ok(invert.value());
        }
        let global = value.global();
        if let Some(unit) = value.downcast::<CSSUnitValue>() {
            if unit.unit_str() == "number" {
                if unit.numeric_value() == 0.0 {
                    return Err(Error::Range(c"Cannot divide by zero".to_owned()));
                }
                return Ok(DomRoot::upcast(CSSUnitValue::new(
                    cx,
                    &global,
                    1.0 / unit.numeric_value(),
                    "number",
                )));
            }
        }
        Ok(DomRoot::upcast(CSSMathInvert::Constructor(
            cx,
            &global,
            None,
            DoubleOrCSSNumericValue::CSSNumericValue(value),
        )))
    }

    fn arithmetic(
        &self,
        cx: &mut JSContext,
        arguments: Vec<DoubleOrCSSNumericValue>,
        operation: ArithmeticOp,
    ) -> Fallible<DomRoot<Self>> {
        let global = self.global();
        let mut arguments = arguments
            .into_iter()
            .map(|value| CSSMathValue::rectify(cx, &global, value))
            .collect::<Vec<_>>();
        if matches!(operation, ArithmeticOp::Sub | ArithmeticOp::Div) {
            arguments = arguments
                .into_iter()
                .map(|value| {
                    if matches!(operation, ArithmeticOp::Sub) {
                        Ok(Self::negate(cx, value))
                    } else {
                        Self::invert(cx, value)
                    }
                })
                .collect::<Fallible<Vec<_>>>()?;
        }

        let mut values = match operation {
            ArithmeticOp::Add | ArithmeticOp::Sub => {
                self.downcast::<CSSMathSum>().map(CSSMathSum::values)
            },
            ArithmeticOp::Mul | ArithmeticOp::Div => self
                .downcast::<CSSMathProduct>()
                .map(CSSMathProduct::values),
            ArithmeticOp::Min => self.downcast::<CSSMathMin>().map(CSSMathMin::values),
            ArithmeticOp::Max => self.downcast::<CSSMathMax>().map(CSSMathMax::values),
        }
        .unwrap_or_else(|| vec![DomRoot::from_ref(self)]);
        values.extend(arguments);

        if matches!(operation, ArithmeticOp::Mul | ArithmeticOp::Div) {
            let units = values
                .iter()
                .map(|value| value.downcast::<CSSUnitValue>())
                .collect::<Option<Vec<_>>>();
            if let Some(units) = units {
                let non_number = units
                    .iter()
                    .filter(|unit| unit.unit_str() != "number")
                    .collect::<Vec<_>>();
                if non_number.len() <= 1 {
                    let unit = non_number
                        .first()
                        .map_or("number", |value| value.unit_str());
                    let value = units
                        .iter()
                        .fold(1.0, |product, item| product * item.numeric_value());
                    return Ok(DomRoot::upcast(CSSUnitValue::new(cx, &global, value, unit)));
                }
            }
            return CSSMathProduct::Constructor(
                cx,
                &global,
                None,
                values
                    .into_iter()
                    .map(DoubleOrCSSNumericValue::CSSNumericValue)
                    .collect(),
            )
            .map(DomRoot::upcast);
        }

        let units = values
            .iter()
            .map(|value| value.downcast::<CSSUnitValue>())
            .collect::<Option<Vec<_>>>();
        if let Some(units) = units {
            if units
                .iter()
                .all(|unit| unit.unit_str() == units[0].unit_str())
            {
                let value = units
                    .iter()
                    .skip(1)
                    .fold(units[0].numeric_value(), |acc, unit| match operation {
                        ArithmeticOp::Add | ArithmeticOp::Sub => acc + unit.numeric_value(),
                        ArithmeticOp::Min => acc.min(unit.numeric_value()),
                        ArithmeticOp::Max => acc.max(unit.numeric_value()),
                        _ => unreachable!(),
                    });
                return Ok(DomRoot::upcast(CSSUnitValue::new(
                    cx,
                    &global,
                    value,
                    units[0].unit_str(),
                )));
            }
        }
        let values = values
            .into_iter()
            .map(DoubleOrCSSNumericValue::CSSNumericValue)
            .collect();
        match operation {
            ArithmeticOp::Add | ArithmeticOp::Sub => {
                CSSMathSum::Constructor(cx, &global, None, values).map(DomRoot::upcast)
            },
            ArithmeticOp::Min => {
                CSSMathMin::Constructor(cx, &global, None, values).map(DomRoot::upcast)
            },
            ArithmeticOp::Max => {
                CSSMathMax::Constructor(cx, &global, None, values).map(DomRoot::upcast)
            },
            _ => unreachable!(),
        }
    }

    fn equal_numeric(&self, other: &Self) -> bool {
        if let (Some(left), Some(right)) = (
            self.downcast::<CSSUnitValue>(),
            other.downcast::<CSSUnitValue>(),
        ) {
            return left.unit_str() == right.unit_str() &&
                left.numeric_value() == right.numeric_value();
        }
        let (Some(left), Some(right)) = (
            self.downcast::<CSSMathValue>(),
            other.downcast::<CSSMathValue>(),
        ) else {
            return false;
        };
        if left.operator() != right.operator() {
            return false;
        }
        let left = left.operands();
        let right = right.operands();
        left.len() == right.len() &&
            left.iter()
                .zip(right.iter())
                .all(|(left, right)| left.equal_numeric(right))
    }
}

impl CSSNumericValueMethods<crate::DomTypeHolder> for CSSNumericValue {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-add>
    fn Add(
        &self,
        cx: &mut JSContext,
        values: Vec<DoubleOrCSSNumericValue>,
    ) -> Fallible<DomRoot<CSSNumericValue>> {
        self.arithmetic(cx, values, ArithmeticOp::Add)
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-sub>
    fn Sub(
        &self,
        cx: &mut JSContext,
        values: Vec<DoubleOrCSSNumericValue>,
    ) -> Fallible<DomRoot<CSSNumericValue>> {
        self.arithmetic(cx, values, ArithmeticOp::Sub)
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-mul>
    fn Mul(
        &self,
        cx: &mut JSContext,
        values: Vec<DoubleOrCSSNumericValue>,
    ) -> Fallible<DomRoot<CSSNumericValue>> {
        self.arithmetic(cx, values, ArithmeticOp::Mul)
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-div>
    fn Div(
        &self,
        cx: &mut JSContext,
        values: Vec<DoubleOrCSSNumericValue>,
    ) -> Fallible<DomRoot<CSSNumericValue>> {
        self.arithmetic(cx, values, ArithmeticOp::Div)
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-min>
    fn Min(
        &self,
        cx: &mut JSContext,
        values: Vec<DoubleOrCSSNumericValue>,
    ) -> Fallible<DomRoot<CSSNumericValue>> {
        self.arithmetic(cx, values, ArithmeticOp::Min)
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-max>
    fn Max(
        &self,
        cx: &mut JSContext,
        values: Vec<DoubleOrCSSNumericValue>,
    ) -> Fallible<DomRoot<CSSNumericValue>> {
        self.arithmetic(cx, values, ArithmeticOp::Max)
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-equals>
    fn Equals(&self, cx: &mut JSContext, values: Vec<DoubleOrCSSNumericValue>) -> bool {
        let global = self.global();
        values.into_iter().all(|value| {
            let value = CSSMathValue::rectify(cx, &global, value);
            self.equal_numeric(&value)
        })
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-type>
    fn Type(&self) -> CSSNumericType {
        let ty = self.numeric_type();
        let exponent = |base| {
            let power = ty.exponent(base);
            (power != 0).then_some(power)
        };
        CSSNumericType {
            length: exponent(NumericBaseType::Length),
            angle: exponent(NumericBaseType::Angle),
            time: exponent(NumericBaseType::Time),
            frequency: exponent(NumericBaseType::Frequency),
            resolution: exponent(NumericBaseType::Resolution),
            flex: exponent(NumericBaseType::Flex),
            percent: exponent(NumericBaseType::Percent),
            percentHint: match ty.percent_hint() {
                Optional::None => None,
                Optional::Some(NumericBaseType::Length) => Some(CSSNumericBaseType::Length),
                Optional::Some(NumericBaseType::Angle) => Some(CSSNumericBaseType::Angle),
                Optional::Some(NumericBaseType::Time) => Some(CSSNumericBaseType::Time),
                Optional::Some(NumericBaseType::Frequency) => Some(CSSNumericBaseType::Frequency),
                Optional::Some(NumericBaseType::Resolution) => Some(CSSNumericBaseType::Resolution),
                Optional::Some(NumericBaseType::Flex) => Some(CSSNumericBaseType::Flex),
                Optional::Some(NumericBaseType::Percent) => Some(CSSNumericBaseType::Percent),
            },
        }
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-to>
    fn To(&self, cx: &mut JSContext, unit: USVString) -> Fallible<DomRoot<CSSUnitValue>> {
        let unit = unit.to_string().to_ascii_lowercase();
        NumericType::try_from_unit(&unit)
            .map_err(|_| Error::Syntax(Some("Unknown CSS unit".into())))?;
        let mut sum = self
            .sum_value()
            .ok_or_else(|| Error::Type(c"CSS value cannot be expressed as a sum".to_owned()))?;
        if sum.len() != 1 {
            return Err(Error::Type(c"CSS value has multiple units".to_owned()));
        }
        let item = sum.remove(0);
        let (value, source_unit) = item
            .unit_value()
            .ok_or_else(|| Error::Type(c"CSS value has compound units".to_owned()))?;
        let value = convert_number(value, source_unit, &unit)
            .ok_or_else(|| Error::Type(c"Incompatible CSS units".to_owned()))?;
        Ok(CSSUnitValue::new(cx, &self.global(), value, &unit))
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-tosum>
    fn ToSum(&self, cx: &mut JSContext, units: Vec<USVString>) -> Fallible<DomRoot<CSSMathSum>> {
        let units = units
            .into_iter()
            .map(|unit| {
                let unit = unit.to_string().to_ascii_lowercase();
                NumericType::try_from_unit(&unit)
                    .map_err(|_| Error::Syntax(Some("Unknown CSS unit".into())))?;
                Ok(unit)
            })
            .collect::<Fallible<Vec<_>>>()?;
        let sum = self
            .sum_value()
            .ok_or_else(|| Error::Type(c"CSS value cannot be expressed as a sum".to_owned()))?;
        let mut values = sum
            .iter()
            .map(|item| {
                let (value, unit) = item
                    .unit_value()
                    .ok_or_else(|| Error::Type(c"CSS value has compound units".to_owned()))?;
                Ok((value, unit.to_owned()))
            })
            .collect::<Fallible<Vec<_>>>()?;
        let result = if units.is_empty() {
            values.sort_by(|left, right| left.1.cmp(&right.1));
            values
        } else {
            let mut result = Vec::new();
            for unit in units {
                let mut total = 0.0;
                let mut remaining = Vec::new();
                for (value, source) in values {
                    if let Some(converted) = convert_number(value, &source, &unit) {
                        total += converted;
                    } else {
                        remaining.push((value, source));
                    }
                }
                values = remaining;
                result.push((total, unit));
            }
            if !values.is_empty() {
                return Err(Error::Type(c"CSS value has leftover units".to_owned()));
            }
            result
        };
        let global = self.global();
        let arguments = result
            .into_iter()
            .map(|(value, unit)| {
                DoubleOrCSSNumericValue::CSSNumericValue(DomRoot::upcast(CSSUnitValue::new(
                    cx, &global, value, &unit,
                )))
            })
            .collect();
        CSSMathSum::Constructor(cx, &global, None, arguments)
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericvalue-parse>
    fn Parse(
        cx: &mut JSContext,
        global: &GlobalScope,
        css_text: USVString,
    ) -> Fallible<DomRoot<CSSNumericValue>> {
        let css_text = css_text.to_string();
        let mut input = Parser::new(&css_text);
        input
            .parse_entirely(|input| parse_math_atom(cx, global, input, false))
            .map_err(|_| Error::Syntax(Some("Invalid CSS numeric value".into())))
    }
}

fn math_parse_error() -> CssParseError<()> {
    CssParseError::custom(())
}

fn parse_math_operator<'i>(
    input: &mut Parser<'i>,
    operators: &str,
) -> Result<char, CssParseError<()>> {
    match input.next()? {
        Token::Delim(operator) if operators.contains(*operator) => Ok(*operator),
        _ => Err(math_parse_error()),
    }
}

fn parse_math_atom<'i>(
    cx: &mut JSContext,
    global: &GlobalScope,
    input: &mut Parser<'i>,
    allow_group: bool,
) -> Result<DomRoot<CSSNumericValue>, CssParseError<()>> {
    let start = input.position();
    let token = input.next()?.clone();
    let unit = match &token {
        Token::Number { .. } => Some("number".to_owned()),
        Token::Percentage { .. } => Some("percent".to_owned()),
        Token::Dimension { unit, .. } => Some(unit.to_ascii_lowercase()),
        _ => None,
    };
    if let Some(unit) = unit {
        let value = parse_numeric_prefix(input.slice_from(start).trim_start())
            .filter(|value| value.is_finite())
            .ok_or_else(math_parse_error)?;
        NumericType::try_from_unit(&unit).map_err(|_| math_parse_error())?;
        return Ok(DomRoot::upcast(CSSUnitValue::new(cx, global, value, &unit)));
    }

    match token {
        Token::Function(name) if name.eq_ignore_ascii_case("calc") => {
            let result = input.parse_nested_block(|input| parse_math_sum(cx, global, input))?;
            if result.is::<CSSUnitValue>() {
                CSSMathSum::Constructor(
                    cx,
                    global,
                    None,
                    vec![DoubleOrCSSNumericValue::CSSNumericValue(result)],
                )
                .map(DomRoot::upcast)
                .map_err(|_| math_parse_error())
            } else {
                Ok(result)
            }
        },
        Token::Function(name)
            if name.eq_ignore_ascii_case("min") || name.eq_ignore_ascii_case("max") =>
        {
            let values = input.parse_nested_block(|input| {
                input.parse_comma_separated(|input| parse_math_sum(cx, global, input))
            })?;
            let arguments = values
                .into_iter()
                .map(DoubleOrCSSNumericValue::CSSNumericValue)
                .collect();
            if name.eq_ignore_ascii_case("min") {
                CSSMathMin::Constructor(cx, global, None, arguments)
                    .map(DomRoot::upcast)
                    .map_err(|_| math_parse_error())
            } else {
                CSSMathMax::Constructor(cx, global, None, arguments)
                    .map(DomRoot::upcast)
                    .map_err(|_| math_parse_error())
            }
        },
        Token::Function(name) if name.eq_ignore_ascii_case("clamp") => {
            let mut values = input.parse_nested_block(|input| {
                input.parse_comma_separated(|input| parse_math_sum(cx, global, input))
            })?;
            if values.len() != 3 {
                return Err(math_parse_error());
            }
            let upper = DoubleOrCSSNumericValue::CSSNumericValue(values.pop().unwrap());
            let value = DoubleOrCSSNumericValue::CSSNumericValue(values.pop().unwrap());
            let lower = DoubleOrCSSNumericValue::CSSNumericValue(values.pop().unwrap());
            CSSMathClamp::Constructor(cx, global, None, lower, value, upper)
                .map(DomRoot::upcast)
                .map_err(|_| math_parse_error())
        },
        Token::ParenthesisBlock if allow_group => {
            input.parse_nested_block(|input| parse_math_sum(cx, global, input))
        },
        Token::Delim('+') if allow_group => parse_math_atom(cx, global, input, true),
        Token::Delim('-') if allow_group => {
            let value = parse_math_atom(cx, global, input, true)?;
            Ok(DomRoot::upcast(CSSMathNegate::Constructor(
                cx,
                global,
                None,
                DoubleOrCSSNumericValue::CSSNumericValue(value),
            )))
        },
        _ => Err(math_parse_error()),
    }
}

fn parse_math_product<'i>(
    cx: &mut JSContext,
    global: &GlobalScope,
    input: &mut Parser<'i>,
) -> Result<DomRoot<CSSNumericValue>, CssParseError<()>> {
    let mut value = parse_math_atom(cx, global, input, true)?;
    while let Ok(operator) = input.try_parse(|input| parse_math_operator(input, "*/")) {
        let other = parse_math_atom(cx, global, input, true)?;
        let argument = DoubleOrCSSNumericValue::CSSNumericValue(other);
        value = if operator == '*' {
            value.Mul(cx, vec![argument])
        } else {
            value.Div(cx, vec![argument])
        }
        .map_err(|_| math_parse_error())?;
    }
    Ok(value)
}

fn parse_math_sum<'i>(
    cx: &mut JSContext,
    global: &GlobalScope,
    input: &mut Parser<'i>,
) -> Result<DomRoot<CSSNumericValue>, CssParseError<()>> {
    let mut values = vec![parse_math_product(cx, global, input)?];
    while let Ok(operator) = input.try_parse(|input| parse_math_operator(input, "+-")) {
        let mut value = parse_math_product(cx, global, input)?;
        if operator == '-' {
            value = DomRoot::upcast(CSSMathNegate::Constructor(
                cx,
                global,
                None,
                DoubleOrCSSNumericValue::CSSNumericValue(value),
            ));
        }
        values.push(value);
    }
    if values.len() == 1 {
        return Ok(values.remove(0));
    }
    let sum = CSSMathSum::Constructor(
        cx,
        global,
        None,
        values
            .into_iter()
            .map(DoubleOrCSSNumericValue::CSSNumericValue)
            .collect(),
    )
    .map_err(|_| math_parse_error())?;
    if let Some(items) = sum.upcast::<CSSNumericValue>().sum_value() {
        if items.len() == 1 {
            if let Some((value, unit)) = items[0].unit_value() {
                return Ok(DomRoot::upcast(CSSUnitValue::new(cx, global, value, unit)));
            }
        }
    }
    Ok(DomRoot::upcast(sum))
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

fn canonical_unit(group: u8) -> &'static str {
    match group {
        0 => "px",
        1 => "deg",
        2 => "s",
        3 => "hz",
        4 => "dppx",
        _ => unreachable!("Unknown canonical CSS unit group"),
    }
}

fn convert_number(value: f64, from: &str, to: &str) -> Option<f64> {
    let converted = if from == to {
        value
    } else {
        let (from_group, from_factor) = canonical_factor(from)?;
        let (to_group, to_factor) = canonical_factor(to)?;
        if from_group != to_group {
            return None;
        }
        value * from_factor / to_factor
    };
    converted.is_finite().then_some(converted)
}
