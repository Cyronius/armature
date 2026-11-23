/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Parsing of the value of the `filter` attribute of canvas rendering contexts.
//!
//! <https://html.spec.whatwg.org/multipage/#dom-context-2d-filter>

use cssparser::{ParseError, Parser, Token, match_ignore_ascii_case};
use servo_canvas_traits::canvas::FilterOperation;
use style::stylesheets::CssRuleType;
use style::values::computed::color::Color as ComputedColor;
use style::values::specified::color::Color;
use style_traits::ParsingMode;

use super::canvas_state::{parse_color, resolve_color};
use crate::css::css::{ANONYMOUS_CONTENT_URL_DATA, parser_context_for_anonymous_content};
use crate::dom::bindings::codegen::Bindings::CanvasRenderingContext2DBinding::CanvasFilterInput;
use crate::dom::bindings::codegen::Bindings::CanvasRenderingContext2DBinding::CanvasFilterPrimitive;
use crate::dom::bindings::codegen::UnionTypes::DoubleOrDoubleSequence;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::html::htmlcanvaselement::HTMLCanvasElement;

type ParseResult<T> = Result<T, ParseError<()>>;

/// Parses `none | <filter-value-list>`, where only the filter functions that need no resources
/// are supported. Returns the operations to apply, none for `none`.
///
/// <https://drafts.fxtf.org/filter-effects/#typedef-filter-value-list>
pub(super) fn parse_filter(
    canvas: Option<&HTMLCanvasElement>,
    string: &str,
) -> Result<Vec<FilterOperation>, ()> {
    let mut parser = Parser::new(string);

    if parser
        .try_parse(|parser| parser.expect_ident_matching("none"))
        .is_ok()
    {
        return parser
            .expect_exhausted()
            .map(|_| Vec::new())
            .map_err(|_| ());
    }

    let mut operations = Vec::new();
    while !parser.is_exhausted() {
        let operation = parser
            .try_parse(|parser| parse_filter_function(canvas, parser))
            .map_err(|_| ())?;
        operations.push(operation);
    }
    if operations.is_empty() {
        return Err(());
    }
    Ok(operations)
}

fn parse_filter_function<'i>(
    canvas: Option<&HTMLCanvasElement>,
    parser: &mut Parser<'i>,
) -> ParseResult<FilterOperation> {
    let name = parser.expect_function()?.to_ascii_lowercase();
    parser.parse_nested_block(|parser| {
        let operation = match name.as_str() {
            "blur" => {
                let deviation = parse_optional(parser, parse_length, 0.0)?;
                if deviation < 0.0 {
                    return Err(ParseError::custom(()));
                }
                FilterOperation::Blur(deviation, deviation)
            },
            "brightness" => FilterOperation::Brightness(parse_amount(parser)?),
            "contrast" => FilterOperation::Contrast(parse_amount(parser)?),
            "grayscale" => FilterOperation::Grayscale(parse_amount(parser)?),
            "hue-rotate" => FilterOperation::HueRotate(parse_optional(parser, parse_angle, 0.0)?),
            "invert" => FilterOperation::Invert(parse_amount(parser)?),
            "opacity" => FilterOperation::Opacity(parse_amount(parser)?),
            "saturate" => FilterOperation::Saturate(parse_amount(parser)?),
            "sepia" => FilterOperation::Sepia(parse_amount(parser)?),
            "drop-shadow" => parse_drop_shadow(canvas, parser)?,
            _ => return Err(ParseError::custom(())),
        };
        parser.expect_exhausted()?;
        Ok(operation)
    })
}

/// Parses what `parse` parses, or returns `default` if there is nothing left to parse.
fn parse_optional<'i>(
    parser: &mut Parser<'i>,
    parse: impl FnOnce(&mut Parser<'i>) -> ParseResult<f64>,
    default: f64,
) -> ParseResult<f64> {
    if parser.is_exhausted() {
        Ok(default)
    } else {
        parse(parser)
    }
}

/// Parses `[ <number> | <percentage> ]?`, which is `1` when omitted.
fn parse_amount<'i>(parser: &mut Parser<'i>) -> ParseResult<f64> {
    let amount = parse_optional(
        parser,
        |parser| match *parser.next()? {
            Token::Number { value, .. } => Ok(value as f64),
            Token::Percentage { unit_value, .. } => Ok(unit_value as f64),
            _ => Err(ParseError::unexpected_token()),
        },
        1.0,
    )?;
    if amount < 0.0 {
        return Err(ParseError::custom(()));
    }
    Ok(amount)
}

/// Parses a `<length>` in pixels. Only absolute units are supported.
fn parse_length<'i>(parser: &mut Parser<'i>) -> ParseResult<f64> {
    match *parser.next()? {
        Token::Number { value: 0.0, .. } => Ok(0.0),
        Token::Dimension {
            value, ref unit, ..
        } => {
            let pixels_per_unit = match_ignore_ascii_case! { unit,
                "px" => 1.0,
                "in" => 96.0,
                "cm" => 96.0 / 2.54,
                "mm" => 96.0 / 25.4,
                "q" => 96.0 / 101.6,
                "pt" => 96.0 / 72.0,
                "pc" => 16.0,
                _ => return Err(ParseError::custom(())),
            };
            Ok(value as f64 * pixels_per_unit)
        },
        _ => Err(ParseError::unexpected_token()),
    }
}

/// Parses an `<angle>` in degrees, which may be a unitless zero.
fn parse_angle<'i>(parser: &mut Parser<'i>) -> ParseResult<f64> {
    match *parser.next()? {
        Token::Number { value: 0.0, .. } => Ok(0.0),
        Token::Dimension {
            value, ref unit, ..
        } => {
            let degrees_per_unit = match_ignore_ascii_case! { unit,
                "deg" => 1.0,
                "grad" => 360.0 / 400.0,
                "rad" => 180.0 / std::f64::consts::PI,
                "turn" => 360.0,
                _ => return Err(ParseError::custom(())),
            };
            Ok(value as f64 * degrees_per_unit)
        },
        _ => Err(ParseError::unexpected_token()),
    }
}

/// Parses `<color>? && <length>{2,3}`.
fn parse_drop_shadow<'i>(
    canvas: Option<&HTMLCanvasElement>,
    parser: &mut Parser<'i>,
) -> ParseResult<FilterOperation> {
    let context = parser_context_for_anonymous_content(
        CssRuleType::Style,
        ParsingMode::DEFAULT,
        &ANONYMOUS_CONTENT_URL_DATA,
    );

    let parse_color = |parser: &mut Parser<'i>| {
        parser.try_parse(|parser| Color::parse_and_compute(&context, parser, None))
    };

    // The color may come before or after the lengths.
    let mut color = parse_color(parser).ok();
    let offset_x = parse_length(parser)?;
    let offset_y = parse_length(parser)?;
    let std_deviation = parser.try_parse(parse_length).unwrap_or(0.0);
    if color.is_none() {
        color = parse_color(parser).ok();
    }
    if std_deviation < 0.0 {
        return Err(ParseError::custom(()));
    }
    let color = resolve_color(canvas, color.unwrap_or_else(ComputedColor::currentcolor));
    Ok(FilterOperation::DropShadow {
        offset_x,
        offset_y,
        std_deviation,
        color,
    })
}

/// Converts the `filter` member of the options of `beginLayer()` to the operations that it asks for.
pub(super) fn layer_filter(
    canvas: Option<&HTMLCanvasElement>,
    input: Option<&CanvasFilterInput>,
) -> Fallible<Vec<FilterOperation>> {
    let primitives: Vec<&CanvasFilterPrimitive> = match input {
        None => return Ok(Vec::new()),
        // Filter functions.
        Some(CanvasFilterInput::String(string)) => {
            return Ok(parse_filter(canvas, &string.str()).unwrap_or_default());
        },
        // One or several filter primitives.
        Some(CanvasFilterInput::CanvasFilterPrimitive(primitive)) => vec![primitive],
        Some(CanvasFilterInput::CanvasFilterPrimitiveSequence(primitives)) => {
            primitives.iter().collect()
        },
    };

    let mut operations = Vec::new();
    for primitive in primitives {
        let name = primitive.name.as_ref().map(|name| name.str().to_string());
        match name.as_deref() {
            Some("gaussianBlur") => {
                let (x, y) = standard_deviation(primitive.stdDeviation.as_ref(), None)?;
                operations.push(FilterOperation::Blur(x, y));
            },
            Some("dropShadow") => {
                let (x, _) = standard_deviation(primitive.stdDeviation.as_ref(), Some(0.0))?;
                let color = parse_color(canvas, &primitive.floodColor)
                    .map_err(|_| Error::Type(c"Invalid floodColor".to_owned()))?;
                let color = color
                    .with_alpha(color.alpha * (*primitive.floodOpacity as f32).clamp(0.0, 1.0));
                operations.push(FilterOperation::DropShadow {
                    offset_x: *primitive.dx,
                    offset_y: *primitive.dy,
                    std_deviation: x,
                    color,
                });
            },
            // Primitives that are not supported are skipped.
            _ => {},
        }
    }
    Ok(operations)
}

/// Converts the `stdDeviation` of a filter primitive: a number, or the numbers for each axis.
fn standard_deviation(
    value: Option<&DoubleOrDoubleSequence>,
    default: Option<f64>,
) -> Fallible<(f64, f64)> {
    let invalid = || Error::Type(c"Invalid stdDeviation".to_owned());
    let numbers = match value {
        None => vec![default.ok_or_else(invalid)?],
        Some(DoubleOrDoubleSequence::Double(number)) => vec![**number],
        Some(DoubleOrDoubleSequence::DoubleSequence(numbers)) => {
            numbers.iter().map(|number| **number).collect()
        },
    };
    match numbers[..] {
        [x] => Ok((x.max(0.0), x.max(0.0))),
        [x, y] => Ok((x.max(0.0), y.max(0.0))),
        _ => Err(invalid()),
    }
}
