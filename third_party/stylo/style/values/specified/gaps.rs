/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Armature (AS-527): Specified types for CSS gap decoration properties.
//!
//! https://drafts.csswg.org/css-gaps-1/

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::typed_om::ToTyped;
use crate::Zero;
use crate::values::generics::gaps::{GenericGapRuleInset, GenericGapRuleList};
use crate::values::specified::{
    BorderSideWidth, BorderStyle, Color, LengthPercentage, PositiveInteger,
};
use cssparser::Parser;
use style_traits::{ParseError, StyleParseErrorKind};

/// A specified `column-rule-color` or `row-rule-color` value.
pub type GapRuleColor = GenericGapRuleList<Color, PositiveInteger>;
/// A specified `column-rule-style` or `row-rule-style` value.
pub type GapRuleStyle = GenericGapRuleList<BorderStyle, PositiveInteger>;
/// A specified `column-rule-width` or `row-rule-width` value.
pub type GapRuleWidth = GenericGapRuleList<BorderSideWidth, PositiveInteger>;
/// A specified gap decoration inset.
pub type GapRuleInset = GenericGapRuleInset<LengthPercentage>;

impl Parse for GapRuleInset {
    fn parse(context: &ParserContext, input: &mut Parser) -> Result<Self, ParseError> {
        if input
            .try_parse(|i| i.expect_ident_matching("overlap-join"))
            .is_ok()
        {
            return Ok(Self::OverlapJoin);
        }
        Ok(Self::LengthPercentage(LengthPercentage::parse(context, input)?))
    }
}

impl GapRuleInset {
    /// The initial value, `0`.
    pub fn zero() -> Self {
        Self::LengthPercentage(LengthPercentage::zero())
    }
}

/// https://drafts.csswg.org/css-gaps-1/#break
#[allow(missing_docs)]
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum GapRuleBreak {
    None,
    Normal,
    Intersection,
}

/// https://drafts.csswg.org/css-gaps-1/#visibility
#[allow(missing_docs)]
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum GapRuleVisibilityItems {
    All,
    Around,
    Between,
    Normal,
}

/// https://drafts.csswg.org/css-gaps-1/#overlap
#[allow(missing_docs)]
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum RuleOverlap {
    RowOverColumn,
    ColumnOverRow,
}

/// One `<gap-rule>` of the `column-rule`, `row-rule` and `rule` shorthands:
/// `<line-width> || <line-style> || <color>`.
#[derive(Clone, Debug, PartialEq)]
pub struct GapRule {
    /// The width, if specified.
    pub width: Option<BorderSideWidth>,
    /// The style, if specified.
    pub style: Option<BorderStyle>,
    /// The color, if specified.
    pub color: Option<Color>,
}

impl Parse for GapRule {
    fn parse(context: &ParserContext, input: &mut Parser) -> Result<Self, ParseError> {
        let mut rule = GapRule {
            width: None,
            style: None,
            color: None,
        };
        loop {
            if rule.width.is_none() {
                if let Ok(width) = input.try_parse(|i| BorderSideWidth::parse(context, i)) {
                    rule.width = Some(width);
                    continue;
                }
            }
            if rule.style.is_none() {
                if let Ok(style) = input.try_parse(|i| BorderStyle::parse(i)) {
                    rule.style = Some(style);
                    continue;
                }
            }
            if rule.color.is_none() {
                if let Ok(color) = input.try_parse(|i| Color::parse(context, i)) {
                    rule.color = Some(color);
                    continue;
                }
            }
            break;
        }
        if rule.width.is_none() && rule.style.is_none() && rule.color.is_none() {
            return Err(ParseError::custom(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(rule)
    }
}

impl ToTyped for GapRule {}
