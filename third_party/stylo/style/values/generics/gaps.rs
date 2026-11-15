/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Armature (AS-527): Generic types for CSS gap decoration properties.
//!
//! https://drafts.csswg.org/css-gaps-1/

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::typed_om::ToTyped;
use crate::OwnedSlice;
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// One entry of a gap decoration value list: a single value, an integer
/// repeater or an automatic repeater.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
#[repr(C, u8)]
pub enum GenericGapRuleListItem<T, I> {
    /// A single value.
    Value(T),
    /// `repeat(<integer [1,∞]>, <value>#)`
    Repeat(I, OwnedSlice<T>),
    /// `repeat(auto, <value>#)`
    AutoRepeat(OwnedSlice<T>),
}

impl<T: ToCss, I: ToCss> ToCss for GenericGapRuleListItem<T, I> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        fn write_values<T: ToCss, W: Write>(values: &[T], dest: &mut CssWriter<W>) -> fmt::Result {
            for value in values {
                dest.write_str(", ")?;
                value.to_css(dest)?;
            }
            dest.write_char(')')
        }
        match self {
            Self::Value(value) => value.to_css(dest),
            Self::Repeat(count, values) => {
                dest.write_str("repeat(")?;
                count.to_css(dest)?;
                write_values(values, dest)
            },
            Self::AutoRepeat(values) => {
                dest.write_str("repeat(auto")?;
                write_values(values, dest)
            },
        }
    }
}

/// A comma-separated list of gap decoration values, as used by the
/// `column-rule-*` and `row-rule-*` longhands.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
#[repr(C)]
pub struct GenericGapRuleList<T, I>(pub OwnedSlice<GenericGapRuleListItem<T, I>>);

impl<T, I> GenericGapRuleList<T, I> {
    /// A list holding a single value.
    pub fn single(value: T) -> Self {
        Self(OwnedSlice::from(vec![GenericGapRuleListItem::Value(value)]))
    }

    /// The entries of the list.
    pub fn items(&self) -> &[GenericGapRuleListItem<T, I>] {
        &self.0
    }
}

impl<T: Parse, I: Parse> Parse for GenericGapRuleList<T, I> {
    fn parse(context: &ParserContext, input: &mut Parser) -> Result<Self, ParseError> {
        let mut items = Vec::new();
        let mut seen_auto = false;
        loop {
            let item = if input
                .try_parse(|input| input.expect_function_matching("repeat"))
                .is_ok()
            {
                input.parse_nested_block(|input| {
                    let count = if input
                        .try_parse(|i| i.expect_ident_matching("auto"))
                        .is_ok()
                    {
                        None
                    } else {
                        Some(I::parse(context, input)?)
                    };
                    input.expect_comma()?;
                    let values = input.parse_comma_separated(|i| T::parse(context, i))?;
                    Ok(match count {
                        Some(count) => GenericGapRuleListItem::Repeat(count, values.into()),
                        None => GenericGapRuleListItem::AutoRepeat(values.into()),
                    })
                })?
            } else {
                GenericGapRuleListItem::Value(T::parse(context, input)?)
            };
            if matches!(item, GenericGapRuleListItem::AutoRepeat(..)) {
                if seen_auto {
                    return Err(ParseError::custom(StyleParseErrorKind::UnspecifiedError));
                }
                seen_auto = true;
            }
            items.push(item);
            if input.try_parse(|input| input.expect_comma()).is_err() {
                break;
            }
        }
        Ok(Self(items.into()))
    }
}

impl<T: ToCss, I: ToCss> ToCss for GenericGapRuleList<T, I> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        let mut first = true;
        for item in self.0.iter() {
            if !first {
                dest.write_str(", ")?;
            }
            first = false;
            item.to_css(dest)?;
        }
        Ok(())
    }
}

impl<T, I> ToTyped for GenericGapRuleList<T, I> {}

/// A gap decoration inset: `<length-percentage> | overlap-join`.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
)]
#[repr(C, u8)]
pub enum GenericGapRuleInset<LP> {
    /// A length or percentage.
    LengthPercentage(LP),
    /// `overlap-join`
    OverlapJoin,
}

impl<LP> ToTyped for GenericGapRuleInset<LP> {}
