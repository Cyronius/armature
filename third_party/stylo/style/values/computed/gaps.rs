/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Armature (AS-527): Computed types for CSS gap decoration properties.
//!
//! https://drafts.csswg.org/css-gaps-1/

use crate::values::computed::{BorderSideWidth, BorderStyle, Color, LengthPercentage, PositiveInteger};
use crate::Zero;
use crate::values::generics::gaps::{GenericGapRuleInset, GenericGapRuleList};

pub use crate::values::specified::gaps::{GapRuleBreak, GapRuleVisibilityItems, RuleOverlap};

/// A computed `column-rule-color` or `row-rule-color` value.
pub type GapRuleColor = GenericGapRuleList<Color, PositiveInteger>;
/// A computed `column-rule-style` or `row-rule-style` value.
pub type GapRuleStyle = GenericGapRuleList<BorderStyle, PositiveInteger>;
/// A computed `column-rule-width` or `row-rule-width` value.
pub type GapRuleWidth = GenericGapRuleList<BorderSideWidth, PositiveInteger>;
/// A computed gap decoration inset.
pub type GapRuleInset = GenericGapRuleInset<LengthPercentage>;

impl GapRuleInset {
    /// The initial value, `0`.
    pub fn zero() -> Self {
        Self::LengthPercentage(LengthPercentage::zero())
    }
}
