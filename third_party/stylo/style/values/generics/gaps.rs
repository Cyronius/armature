/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Armature (AS-527): Generic types for CSS gap decoration properties.
//!
//! https://drafts.csswg.org/css-gaps-1/

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::typed_om::ToTyped;
use crate::values::animated::{Animate, Procedure, ToAnimatedZero};
use crate::values::distance::{ComputeSquaredDistance, SquaredDistance};
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

/// A gap decoration list split around its automatic repeater, with integer
/// repeaters expanded.
///
/// https://drafts.csswg.org/css-gaps-1/#interpolation
struct ExpandedGapRuleList<'a, T> {
    leading: Vec<&'a T>,
    auto: Option<&'a [T]>,
    trailing: Vec<&'a T>,
}

impl<T, I: Clone + Into<i32>> GenericGapRuleList<T, I> {
    fn expanded(&self) -> ExpandedGapRuleList<'_, T> {
        let mut result = ExpandedGapRuleList {
            leading: Vec::new(),
            auto: None,
            trailing: Vec::new(),
        };
        for item in self.items() {
            let target = if result.auto.is_some() {
                &mut result.trailing
            } else {
                &mut result.leading
            };
            match item {
                GenericGapRuleListItem::Value(value) => target.push(value),
                GenericGapRuleListItem::Repeat(count, values) => {
                    for _ in 0..count.clone().into().max(1) {
                        target.extend(values.iter());
                    }
                },
                GenericGapRuleListItem::AutoRepeat(values) => result.auto = Some(values),
            }
        }
        result
    }
}

fn animate_repeatable<T: Animate>(
    left: &[&T],
    right: &[&T],
    procedure: Procedure,
) -> Result<Vec<T>, ()> {
    if left.is_empty() || right.is_empty() {
        return Err(());
    }
    let len = num_integer::lcm(left.len(), right.len());
    left.iter()
        .cycle()
        .zip(right.iter().cycle())
        .take(len)
        .map(|(left, right)| left.animate(right, procedure))
        .collect()
}

fn animate_pairwise<T: Animate>(left: &[&T], right: &[&T], procedure: Procedure) -> Result<Vec<T>, ()> {
    if left.len() != right.len() {
        return Err(());
    }
    left.iter()
        .zip(right)
        .map(|(left, right)| left.animate(right, procedure))
        .collect()
}

impl<T: Animate + Clone, I: Clone + Into<i32>> Animate for GenericGapRuleList<T, I> {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        let (left, right) = (self.expanded(), other.expanded());
        let values = |values: Vec<T>| values.into_iter().map(GenericGapRuleListItem::Value);
        match (left.auto, right.auto) {
            (None, None) => {
                let mut all_left = left.leading;
                all_left.extend(left.trailing);
                let mut all_right = right.leading;
                all_right.extend(right.trailing);
                let result = animate_repeatable(&all_left, &all_right, procedure)?;
                Ok(Self(values(result).collect::<Vec<_>>().into()))
            },
            (Some(left_auto), Some(right_auto)) => {
                let leading = animate_pairwise(&left.leading, &right.leading, procedure)?;
                let trailing = animate_pairwise(&left.trailing, &right.trailing, procedure)?;
                let left_auto: Vec<&T> = left_auto.iter().collect();
                let right_auto: Vec<&T> = right_auto.iter().collect();
                let auto = animate_repeatable(&left_auto, &right_auto, procedure)?;
                let mut items: Vec<_> = values(leading).collect();
                items.push(GenericGapRuleListItem::AutoRepeat(auto.into()));
                items.extend(values(trailing));
                Ok(Self(items.into()))
            },
            _ => Err(()),
        }
    }
}

impl<T: ComputeSquaredDistance, I: Clone + Into<i32>> ComputeSquaredDistance
    for GenericGapRuleList<T, I>
{
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        let (left, right) = (self.expanded(), other.expanded());
        let distance = |left: &[&T], right: &[&T]| -> Result<SquaredDistance, ()> {
            if left.is_empty() || right.is_empty() {
                return Err(());
            }
            let len = num_integer::lcm(left.len(), right.len());
            left.iter()
                .cycle()
                .zip(right.iter().cycle())
                .take(len)
                .map(|(left, right)| left.compute_squared_distance(right))
                .sum()
        };
        let mut all_left = left.leading;
        all_left.extend(left.trailing);
        let mut all_right = right.leading;
        all_right.extend(right.trailing);
        match (left.auto, right.auto) {
            (None, None) => distance(&all_left, &all_right),
            _ => Err(()),
        }
    }
}

impl<T: ToAnimatedZero, I: Clone> ToAnimatedZero for GenericGapRuleList<T, I> {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        let zero = |values: &[T]| -> Result<OwnedSlice<T>, ()> {
            Ok(values
                .iter()
                .map(|value| value.to_animated_zero())
                .collect::<Result<Vec<_>, _>>()?
                .into())
        };
        Ok(Self(
            self.items()
                .iter()
                .map(|item| {
                    Ok(match item {
                        GenericGapRuleListItem::Value(value) => {
                            GenericGapRuleListItem::Value(value.to_animated_zero()?)
                        },
                        GenericGapRuleListItem::Repeat(count, values) => {
                            GenericGapRuleListItem::Repeat(count.clone(), zero(values)?)
                        },
                        GenericGapRuleListItem::AutoRepeat(values) => {
                            GenericGapRuleListItem::AutoRepeat(zero(values)?)
                        },
                    })
                })
                .collect::<Result<Vec<_>, ()>>()?
                .into(),
        ))
    }
}

impl<LP: Animate> Animate for GenericGapRuleInset<LP> {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        match (self, other) {
            (Self::LengthPercentage(left), Self::LengthPercentage(right)) => {
                Ok(Self::LengthPercentage(left.animate(right, procedure)?))
            },
            (Self::OverlapJoin, Self::OverlapJoin) => Ok(Self::OverlapJoin),
            _ => Err(()),
        }
    }
}

impl<LP: ComputeSquaredDistance> ComputeSquaredDistance for GenericGapRuleInset<LP> {
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        match (self, other) {
            (Self::LengthPercentage(left), Self::LengthPercentage(right)) => {
                left.compute_squared_distance(right)
            },
            (Self::OverlapJoin, Self::OverlapJoin) => Ok(SquaredDistance::from_sqrt(0.)),
            _ => Err(()),
        }
    }
}

impl<LP: ToAnimatedZero> ToAnimatedZero for GenericGapRuleInset<LP> {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        match self {
            Self::LengthPercentage(value) => Ok(Self::LengthPercentage(value.to_animated_zero()?)),
            Self::OverlapJoin => Err(()),
        }
    }
}
