/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use script_bindings::codegen::GenericBindings::WindowBinding::WindowMethods;
use script_bindings::num::Finite;
use script_bindings::reflector::Reflector;
use time::Duration;

use crate::dom::bindings::codegen::Bindings::AnimationTimelineBinding::AnimationTimelineMethods;
use crate::dom::bindings::codegen::UnionTypes::DoubleOrCSSNumericValue;
use crate::dom::bindings::reflector::DomGlobal;

/// <https://drafts.csswg.org/web-animations-1/#animationtimeline>
#[dom_struct]
pub(crate) struct AnimationTimeline {
    reflector_: Reflector,
    /// <https://drafts.csswg.org/web-animations-1/#dom-animationtimeline-currenttime>
    ///
    /// The current time of this [`AnimationTimeline`] expressed as a [`time::Duration`] since the
    /// Document's "time origin." Note that this Duration may be negative.
    #[no_trace]
    current_time: Cell<Duration>,
    /// Constructed document timelines follow the default timeline with this offset.
    /// `None` identifies the default timeline, whose sampled time is stored above.
    #[no_trace]
    document_origin_offset: Option<Duration>,
}

impl AnimationTimeline {
    pub(crate) fn new_inherited(
        current_time: Duration,
        document_origin_offset: Option<Duration>,
    ) -> Self {
        Self {
            reflector_: Reflector::new(),
            current_time: Cell::new(current_time),
            document_origin_offset,
        }
    }

    pub(crate) fn current_time_in_seconds(&self) -> f64 {
        match self.document_origin_offset {
            Some(offset) => {
                self.global()
                    .as_window()
                    .Document()
                    .current_animation_timeline_value() -
                    offset.as_seconds_f64()
            },
            None => self.current_time.get().as_seconds_f64(),
        }
    }

    pub(crate) fn set_current_time(&self, duration: Duration) {
        self.current_time.set(duration);
    }

    pub(crate) fn advance_specific(&self, by: Duration) {
        self.current_time.set(self.current_time.get() + by);
    }
}

impl AnimationTimelineMethods<crate::DomTypeHolder> for AnimationTimeline {
    /// <https://drafts.csswg.org/web-animations-1/#dom-animationtimeline-currenttime>
    fn GetCurrentTime(&self) -> Option<Finite<f64>> {
        Finite::new(self.current_time_in_seconds() * 1000.0)
    }

    /// <https://drafts.csswg.org/web-animations-2/#dom-animationtimeline-duration>
    fn GetDuration(&self) -> Option<DoubleOrCSSNumericValue> {
        None
    }
}
