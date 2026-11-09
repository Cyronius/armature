/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use cssparser::{ParseError as CssParseError, Parser as CssParser, Token};
use dom_struct::dom_struct;
use html5ever::{LocalName, local_name};
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedLengthBinding::SVGAnimatedLengthMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::element::Element;
use crate::dom::node::NodeTraits;
use crate::dom::svg::svganimationelement::trim_one_trailing_semicolon;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svglength::{ElementLengthContext, SVGLength};
use crate::dom::svg::svgtypes::{SVGLengthContext, SVGLengthDirection, SVGLengthValue};

#[derive(Clone, Copy)]
enum CalcAmount {
    Number(f32),
    Length(f32),
}

impl CalcAmount {
    fn add(self, other: Self, subtract: bool) -> Option<Self> {
        let sign = if subtract { -1.0 } else { 1.0 };
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => Some(Self::Number(a + sign * b)),
            (Self::Length(a), Self::Length(b)) => Some(Self::Length(a + sign * b)),
            _ => None,
        }
    }

    fn multiply(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => Some(Self::Number(a * b)),
            (Self::Number(a), Self::Length(b)) | (Self::Length(b), Self::Number(a)) => {
                Some(Self::Length(a * b))
            },
            _ => None,
        }
    }

    fn divide(self, other: Self) -> Option<Self> {
        let Self::Number(denominator) = other else {
            return None;
        };
        if denominator == 0.0 {
            return None;
        }
        match self {
            Self::Number(value) => Some(Self::Number(value / denominator)),
            Self::Length(value) => Some(Self::Length(value / denominator)),
        }
    }
}

fn calc_operator<'i>(
    input: &mut CssParser<'i>,
    operators: &str,
) -> Result<char, CssParseError<()>> {
    match input.next()? {
        Token::Delim(operator) if operators.contains(*operator) => Ok(*operator),
        _ => Err(CssParseError::custom(())),
    }
}

fn calc_atom<'i>(
    cx: &mut JSContext,
    input: &mut CssParser<'i>,
    context: &dyn SVGLengthContext,
) -> Result<CalcAmount, CssParseError<()>> {
    match input.next()?.clone() {
        Token::Number { value, .. } if value.is_finite() => Ok(CalcAmount::Number(value)),
        Token::Percentage { unit_value, .. } => {
            let length = SVGLengthValue::percentage(unit_value * 100.0)
                .to_user_units(cx, context)
                .filter(|value| value.is_finite())
                .ok_or_else(|| CssParseError::custom(()))?;
            Ok(CalcAmount::Length(length))
        },
        Token::Dimension { value, unit, .. } => {
            let length = SVGLengthValue::parse(&format!("{value}{unit}"))
                .and_then(|value| value.to_user_units(cx, context))
                .filter(|value| value.is_finite())
                .ok_or_else(|| CssParseError::custom(()))?;
            Ok(CalcAmount::Length(length))
        },
        Token::Function(name) if name.eq_ignore_ascii_case("calc") => {
            input.parse_nested_block(|input| calc_sum(cx, input, context))
        },
        Token::ParenthesisBlock => input.parse_nested_block(|input| calc_sum(cx, input, context)),
        Token::Delim('+') => calc_atom(cx, input, context),
        Token::Delim('-') => match calc_atom(cx, input, context)? {
            CalcAmount::Number(value) => Ok(CalcAmount::Number(-value)),
            CalcAmount::Length(value) => Ok(CalcAmount::Length(-value)),
        },
        _ => Err(CssParseError::custom(())),
    }
}

fn calc_product<'i>(
    cx: &mut JSContext,
    input: &mut CssParser<'i>,
    context: &dyn SVGLengthContext,
) -> Result<CalcAmount, CssParseError<()>> {
    let mut value = calc_atom(cx, input, context)?;
    while let Ok(operator) = input.try_parse(|input| calc_operator(input, "*/")) {
        let other = calc_atom(cx, input, context)?;
        value = if operator == '*' {
            value.multiply(other)
        } else {
            value.divide(other)
        }
        .ok_or_else(|| CssParseError::custom(()))?;
    }
    Ok(value)
}

fn calc_sum<'i>(
    cx: &mut JSContext,
    input: &mut CssParser<'i>,
    context: &dyn SVGLengthContext,
) -> Result<CalcAmount, CssParseError<()>> {
    let mut value = calc_product(cx, input, context)?;
    while let Ok(operator) = input.try_parse(|input| calc_operator(input, "+-")) {
        let other = calc_product(cx, input, context)?;
        value = value
            .add(other, operator == '-')
            .ok_or_else(|| CssParseError::custom(()))?;
    }
    Ok(value)
}

fn parse_animation_length(
    cx: &mut JSContext,
    context: &dyn SVGLengthContext,
    value: &str,
) -> Option<SVGLengthValue> {
    if let Some(length) = SVGLengthValue::parse(value) {
        return Some(length);
    }
    let mut input = CssParser::new(value);
    let amount = input
        .parse_entirely(|input| {
            input.expect_function_matching("calc")?;
            input.parse_nested_block(|input| calc_sum(cx, input, context))
        })
        .ok()?;
    let value = match amount {
        CalcAmount::Number(value) | CalcAmount::Length(value) => value,
    };
    value.is_finite().then_some(SVGLengthValue::number(value))
}

/// Interpolate in user units, retaining the endpoint unit appropriate to the
/// current half of the interval for the animated SVGLength's `unitType`.
fn interpolate_lengths(
    cx: &mut JSContext,
    context: &dyn SVGLengthContext,
    from: SVGLengthValue,
    to: SVGLengthValue,
    progress: f32,
) -> Option<SVGLengthValue> {
    if from.unit_type == to.unit_type {
        return Some(SVGLengthValue::new(
            from.value + (to.value - from.value) * progress,
            from.unit_type,
        ));
    }
    let from_units = from.to_user_units(cx, context)?;
    let to_units = to.to_user_units(cx, context)?;
    let unit_type = if progress < 0.5 {
        from.unit_type
    } else {
        to.unit_type
    };
    SVGLengthValue::from_user_units(
        cx,
        from_units + (to_units - from_units) * progress,
        unit_type,
        context,
    )
}

fn add_lengths(
    cx: &mut JSContext,
    context: &dyn SVGLengthContext,
    underlying: SVGLengthValue,
    effect: SVGLengthValue,
) -> Option<SVGLengthValue> {
    let sum = underlying.to_user_units(cx, context)? + effect.to_user_units(cx, context)?;
    SVGLengthValue::from_user_units(cx, sum, underlying.unit_type, context)
}

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedLength>
///
/// The base value is not stored: it is parsed from the content attribute when it is read,
/// and serialized into the content attribute when it is changed, so that the two always
/// agree. SVG `<animate>` children can supply a composed animated length value.
#[dom_struct]
pub(crate) struct SVGAnimatedLength {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    direction: SVGLengthDirection,
    /// The value when the attribute is missing or invalid.
    initial: SVGLengthValue,
    base_val: MutNullableDom<SVGLength>,
    anim_val: MutNullableDom<SVGLength>,
}

impl SVGAnimatedLength {
    fn new_inherited(
        element: &SVGElement,
        attribute: LocalName,
        direction: SVGLengthDirection,
        initial: SVGLengthValue,
    ) -> Self {
        Self {
            reflector_: Reflector::new(),
            element: Dom::from_ref(element),
            attribute,
            direction,
            initial,
            base_val: Default::default(),
            anim_val: Default::default(),
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: LocalName,
        direction: SVGLengthDirection,
        initial: SVGLengthValue,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self::new_inherited(element, attribute, direction, initial)),
            &*element.owner_window(),
        )
    }

    pub(crate) fn element(&self) -> &SVGElement {
        &self.element
    }

    pub(crate) fn direction(&self) -> SVGLengthDirection {
        self.direction
    }

    /// The base value, parsed from the content attribute.
    pub(crate) fn base_value(&self) -> SVGLengthValue {
        Self::base_value_for(&self.element, &self.attribute, self.initial)
    }

    fn base_value_for(
        element: &SVGElement,
        attribute: &LocalName,
        initial: SVGLengthValue,
    ) -> SVGLengthValue {
        // Negative values of the lengths that cannot be negative are invalid.
        let non_negative = matches!(
            &**attribute,
            "width" | "height" | "r" | "rx" | "ry" | "fr" | "markerWidth" | "markerHeight"
        );
        element
            .svg_attribute(attribute)
            .and_then(|value| SVGLengthValue::parse(&value))
            .filter(|value| !non_negative || value.value >= 0.0)
            .unwrap_or(initial)
    }

    /// Sample length animations at the SVG root's current time. An animation's
    /// effect is composed over the value supplied by earlier animations.
    pub(crate) fn animated_value(&self, cx: &mut JSContext) -> SVGLengthValue {
        Self::sample_value(
            cx,
            &self.element,
            &self.attribute,
            self.direction,
            self.initial,
        )
    }

    /// Sample an attribute for both its DOM `animVal` and rendered geometry.
    pub(crate) fn sample_value(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: &LocalName,
        direction: SVGLengthDirection,
        initial: SVGLengthValue,
    ) -> SVGLengthValue {
        let mut result = Self::base_value_for(element, attribute, initial);
        let context = ElementLengthContext { element, direction };

        let mut animations = element
            .owner_document()
            .smil_animations_targeting(cx, element);
        animations.sort_by(|left, right| {
            left.last_begin_time()
                .unwrap_or(f64::NEG_INFINITY)
                .total_cmp(&right.last_begin_time().unwrap_or(f64::NEG_INFINITY))
        });
        for animation in animations {
            let child = animation.upcast::<Element>();
            if child.local_name() == &local_name!("set") {
                if animation
                    .upcast::<SVGElement>()
                    .svg_attribute(&local_name!("attributeName"))
                    .as_deref() ==
                    Some(attribute.as_ref()) &&
                    animation.set_is_active() &&
                    let Some(to) = animation
                        .upcast::<SVGElement>()
                        .svg_attribute(&local_name!("to"))
                        .and_then(|value| parse_animation_length(cx, &context, &value))
                {
                    result = to;
                }
                continue;
            }
            if child.local_name() != &local_name!("animate") {
                continue;
            }
            if animation
                .upcast::<SVGElement>()
                .svg_attribute(&local_name!("attributeName"))
                .as_deref() !=
                Some(attribute.as_ref())
            {
                continue;
            }
            let Some((progress, iteration)) = animation.sample_iteration() else {
                continue;
            };
            let element = animation.upcast::<SVGElement>();
            let additive =
                element.svg_attribute(&local_name!("additive")).as_deref() == Some("sum");
            let accumulate =
                element.svg_attribute(&local_name!("accumulate")).as_deref() == Some("sum");
            if let Some(values) = element.svg_attribute(&local_name!("values")) {
                let values: Option<Vec<_>> = trim_one_trailing_semicolon(&values)
                    .split(';')
                    .map(|value| parse_animation_length(cx, &context, value))
                    .collect();
                let Some(values) = values.filter(|values| !values.is_empty()) else {
                    continue;
                };
                let mut effect = if values.len() == 1 {
                    values[0]
                } else {
                    let Some((index, fraction)) =
                        animation.sample_value_interval(values.len(), progress)
                    else {
                        continue;
                    };
                    let Some(effect) = interpolate_lengths(
                        cx,
                        &context,
                        values[index],
                        values[index + 1],
                        fraction,
                    ) else {
                        continue;
                    };
                    effect
                };
                if accumulate && iteration > 0 {
                    let final_value = values[values.len() - 1];
                    let scaled = SVGLengthValue::new(
                        final_value.value * iteration as f32,
                        final_value.unit_type,
                    );
                    let Some(sum) = add_lengths(cx, &context, effect, scaled) else {
                        continue;
                    };
                    effect = sum;
                }
                if additive {
                    let Some(sum) = add_lengths(cx, &context, result, effect) else {
                        continue;
                    };
                    result = sum;
                } else {
                    result = effect;
                }
                continue;
            }
            let from_attribute = element.svg_attribute(&local_name!("from"));
            let to_attribute = element.svg_attribute(&local_name!("to"));
            let by_attribute = element.svg_attribute(&local_name!("by"));
            let from = from_attribute
                .as_deref()
                .and_then(|value| parse_animation_length(cx, &context, value));
            let to = to_attribute
                .as_deref()
                .and_then(|value| parse_animation_length(cx, &context, value));
            let by = by_attribute
                .as_deref()
                .and_then(|value| parse_animation_length(cx, &context, value));
            if (from_attribute.is_some() && from.is_none()) ||
                (to_attribute.is_some() && to.is_none()) ||
                (by_attribute.is_some() && by.is_none())
            {
                continue;
            }

            let Some((_, fraction)) = animation.sample_value_interval(2, progress) else {
                continue;
            };

            let (mut effect, additive, final_value) = match (from, to, by) {
                (Some(from), Some(to), _) => {
                    let Some(effect) = interpolate_lengths(cx, &context, from, to, fraction) else {
                        continue;
                    };
                    (effect, additive, Some(to))
                },
                (Some(from), None, Some(by)) => {
                    let Some(by_units) = by.to_user_units(cx, &context) else {
                        continue;
                    };
                    let Some(from_units) = from.to_user_units(cx, &context) else {
                        continue;
                    };
                    let Some(to) = SVGLengthValue::from_user_units(
                        cx,
                        from_units + by_units,
                        from.unit_type,
                        &context,
                    ) else {
                        continue;
                    };
                    let Some(effect) = interpolate_lengths(cx, &context, from, to, fraction) else {
                        continue;
                    };
                    (effect, additive, Some(to))
                },
                (None, None, Some(by)) => {
                    // A by-only animation is additive even with additive="replace".
                    (
                        SVGLengthValue::new(by.value * fraction, by.unit_type),
                        true,
                        Some(by),
                    )
                },
                (None, Some(to), _) => {
                    let Some(effect) = interpolate_lengths(cx, &context, result, to, fraction)
                    else {
                        continue;
                    };
                    (effect, false, None)
                },
                _ => continue,
            };
            if accumulate &&
                iteration > 0 &&
                let Some(final_value) = final_value
            {
                let scaled = SVGLengthValue::new(
                    final_value.value * iteration as f32,
                    final_value.unit_type,
                );
                let Some(sum) = add_lengths(cx, &context, effect, scaled) else {
                    continue;
                };
                effect = sum;
            }
            if additive {
                let Some(sum) = add_lengths(cx, &context, result, effect) else {
                    continue;
                };
                result = sum;
            } else {
                result = effect;
            }
        }
        result
    }

    /// Set the base value, by setting the content attribute.
    pub(crate) fn set_base_value(&self, cx: &mut JSContext, value: SVGLengthValue) {
        self.element
            .set_svg_attribute(cx, &self.attribute, value.to_string());
    }
}

impl SVGAnimatedLengthMethods<crate::DomTypeHolder> for SVGAnimatedLength {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedLength__baseVal>
    fn BaseVal(&self, cx: &mut JSContext) -> DomRoot<SVGLength> {
        if let Some(length) = self.base_val.get() {
            return length;
        }
        let length = SVGLength::new_reflected(cx, self, false);
        self.base_val.set(Some(&length));
        length
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedLength__animVal>
    fn AnimVal(&self, cx: &mut JSContext) -> DomRoot<SVGLength> {
        if let Some(length) = self.anim_val.get() {
            return length;
        }
        let length = SVGLength::new_reflected(cx, self, true);
        self.anim_val.set(Some(&length));
        length
    }
}
