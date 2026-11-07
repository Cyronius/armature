/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use cssparser::Parser;
use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::cell::DomRefCell;
use script_bindings::codegen::GenericBindings::AnimationEffectBinding::{
    AnimationEffectMethods, ComputedEffectTiming, EffectTiming, FillMode, OptionalEffectTiming,
    PlaybackDirection,
};
use script_bindings::codegen::GenericBindings::WindowBinding::WindowMethods;
use script_bindings::error::{Error, Fallible};
use script_bindings::num::Finite;
use script_bindings::reflector::Reflector;
use script_bindings::root::{Dom, DomRoot};
use script_bindings::str::DOMString;
use style::parser::Parse;
use style::stylesheets::CssRuleType;
use style::values::generics::easing::{BeforeFlag, TimingKeyword};
use style::values::specified::TimingFunction;
use style_traits::{ParsingMode, ToCss};

use crate::css::css::parser_context_for_document;
use crate::dom::Window;
use crate::dom::animation::Animation;
use crate::dom::bindings::codegen::UnionTypes::UnrestrictedDoubleOrString;
use crate::dom::bindings::root::MutNullableDom;

/// <https://drafts.csswg.org/web-animations-1/#animationeffect>
#[dom_struct]
pub(crate) struct AnimationEffect {
    reflector: Reflector,

    /// The window that this `AnimationEffect` was constructed in.
    window: Dom<Window>,

    specified_timing_properties: DomRefCell<SpecifiedTimingProperties>,

    /// <https://drafts.csswg.org/web-animations-1/#animation-effect-associated-animation>
    associated_animation: MutNullableDom<Animation>,
}

/// <https://drafts.csswg.org/web-animations-1/#animation-effect-phases-and-states>
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum EffectPhase {
    Before,
    Active,
    After,
}

/// The results of the timing calculations of an animation effect at some local time.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EffectTimes {
    /// <https://drafts.csswg.org/web-animations-1/#active-time>
    pub(crate) active_time: Option<f64>,
    /// <https://drafts.csswg.org/web-animations-1/#current-iteration>
    pub(crate) current_iteration: Option<f64>,
    /// <https://drafts.csswg.org/web-animations-1/#iteration-progress>
    pub(crate) iteration_progress: Option<f64>,
    /// <https://drafts.csswg.org/web-animations-1/#before-flag>
    pub(crate) before_flag: bool,
    /// The phase of the effect, or `None` if its local time is unresolved.
    pub(crate) phase: Option<EffectPhase>,
}

/// The timing properties of an effect that the `animation-*` and `transition-*` properties
/// set, in milliseconds.
#[derive(Clone, Copy, JSTraceable, MallocSizeOf, PartialEq)]
pub(crate) struct CssEffectTiming {
    pub(crate) duration: f64,
    pub(crate) delay: f64,
    pub(crate) iterations: f64,
    pub(crate) direction: PlaybackDirection,
    pub(crate) fill: FillMode,
}

/// Flags for the timing properties that script changed with `updateTiming()`, which
/// style no longer changes.
pub(crate) mod overridden {
    pub(crate) const PLAY_STATE: u8 = 1 << 0;
    pub(crate) const DURATION: u8 = 1 << 1;
    pub(crate) const DELAY: u8 = 1 << 2;
    pub(crate) const ITERATIONS: u8 = 1 << 3;
    pub(crate) const DIRECTION: u8 = 1 << 4;
    pub(crate) const FILL: u8 = 1 << 5;
}

#[derive(Clone, JSTraceable, MallocSizeOf)]
struct SpecifiedTimingProperties {
    /// <https://drafts.csswg.org/web-animations-1/#start-delay>
    start_delay: Finite<f64>,

    /// <https://drafts.csswg.org/web-animations-1/#end-delay>
    end_delay: Finite<f64>,

    /// <https://drafts.csswg.org/web-animations-1/#fill-mode>
    fill_mode: FillMode,

    /// <https://drafts.csswg.org/web-animations-1/#iteration-count>
    iteration_count: f64,

    /// <https://drafts.csswg.org/web-animations-1/#iteration-start>
    iteration_start: Finite<f64>,

    /// <https://drafts.csswg.org/web-animations-1/#iteration-duration>
    iteration_duration: IterationDurationOrAuto,

    /// <https://drafts.csswg.org/web-animations-1/#playback-direction>
    playback_direction: PlaybackDirection,

    /// <https://drafts.csswg.org/css-easing-2/#easing-function>
    #[no_trace]
    easing_function: TimingFunction,
}

impl AnimationEffect {
    pub(crate) fn new_inherited(window: &Window) -> Self {
        Self {
            reflector: Reflector::new(),
            window: Dom::from_ref(window),

            // The default values of the timing properties specified here don't matter.
            // There is no way to construct a AnimationEffect without subsequently initializing them,
            // even if they're not passed to the constructor.
            specified_timing_properties: DomRefCell::new(SpecifiedTimingProperties {
                start_delay: Default::default(),
                end_delay: Default::default(),
                fill_mode: FillMode::None,
                iteration_count: Default::default(),
                iteration_start: Default::default(),
                iteration_duration: IterationDurationOrAuto::Auto,
                playback_direction: PlaybackDirection::Normal,
                easing_function: TimingFunction::Keyword(TimingKeyword::Linear),
            }),
            associated_animation: Default::default(),
        }
    }

    pub(crate) fn window(&self) -> &Window {
        &self.window
    }

    /// <https://drafts.csswg.org/web-animations-1/#animation-effect-associated-animation>
    pub(crate) fn associated_animation(&self) -> Option<DomRoot<Animation>> {
        self.associated_animation.get()
    }

    pub(crate) fn set_associated_animation(&self, animation: Option<&Animation>) {
        self.associated_animation.set(animation);
    }

    /// <https://drafts.csswg.org/web-animations-1/#active-duration>
    pub(crate) fn active_duration(&self) -> f64 {
        let timing = self.specified_timing_properties.borrow();
        let iteration_duration = timing.iteration_duration.computed_value();
        // > The active duration is calculated as follows:
        // > active duration = iteration duration × iteration count
        // > If either the iteration duration or iteration count are zero, the active duration is zero.
        if iteration_duration == 0.0 || timing.iteration_count == 0.0 {
            return 0.0;
        }
        iteration_duration * timing.iteration_count
    }

    /// <https://drafts.csswg.org/web-animations-1/#start-delay>
    pub(crate) fn start_delay(&self) -> f64 {
        *self.specified_timing_properties.borrow().start_delay
    }

    /// <https://drafts.csswg.org/web-animations-1/#iteration-duration>
    pub(crate) fn iteration_duration(&self) -> f64 {
        self.specified_timing_properties
            .borrow()
            .iteration_duration
            .computed_value()
    }

    /// <https://drafts.csswg.org/web-animations-1/#iteration-start>
    pub(crate) fn iteration_start(&self) -> f64 {
        *self.specified_timing_properties.borrow().iteration_start
    }

    /// The active time at `local_time` as if the fill mode were `both`, used for the
    /// elapsed time of cancel events of CSS animations and transitions.
    pub(crate) fn active_time_with_fill_both(&self, local_time: f64) -> f64 {
        (local_time - self.start_delay())
            .min(self.active_duration())
            .max(0.0)
    }

    /// Set the timing properties that the `animation-*` or `transition-*` properties set,
    /// except those in `skip`, a set of [`overridden`] flags.
    pub(crate) fn set_css_timing(
        &self,
        timing: &CssEffectTiming,
        skip: u8,
        easing: Option<TimingFunction>,
    ) {
        let wanted = |flag: u8| skip & flag == 0;
        let input = OptionalEffectTiming {
            delay: wanted(overridden::DELAY).then(|| Finite::wrap(timing.delay)),
            direction: wanted(overridden::DIRECTION).then_some(timing.direction),
            duration: wanted(overridden::DURATION)
                .then(|| UnrestrictedDoubleOrString::UnrestrictedDouble(timing.duration)),
            easing: None,
            endDelay: None,
            fill: wanted(overridden::FILL).then_some(timing.fill),
            iterationStart: None,
            iterations: wanted(overridden::ITERATIONS).then_some(timing.iterations),
        };
        let _ = self.update_the_timing_properties(&input);
        if let Some(easing) = easing {
            self.specified_timing_properties
                .borrow_mut()
                .easing_function = easing;
        }
    }

    /// <https://drafts.csswg.org/web-animations-1/#end-time>
    pub(crate) fn end_time(&self) -> f64 {
        let (start_delay, end_delay) = {
            let timing = self.specified_timing_properties.borrow();
            (*timing.start_delay, *timing.end_delay)
        };
        // > The end time of an animation effect is the result of evaluating
        // > max(start delay + active duration + end delay, 0).
        (start_delay + self.active_duration() + end_delay).max(0.0)
    }

    /// Calculate the phase, active time, current iteration and iteration progress of this
    /// effect for a local time, when its animation plays at `playback_rate`.
    ///
    /// <https://drafts.csswg.org/web-animations-1/#core-animation-model-calculations>
    pub(crate) fn compute_timing(
        &self,
        local_time: Option<f64>,
        playback_rate: f64,
    ) -> EffectTimes {
        self.compute_timing_with_endpoint(local_time, playback_rate, false)
    }

    /// Like [`Self::compute_timing`], but when `inclusive_endpoint` is true an effect whose
    /// local time is exactly at the end of its active interval, in the playback direction,
    /// is in its active phase. `commitStyles()` commits the final value of a finished
    /// animation that does not fill this way, as other engines do.
    pub(crate) fn compute_timing_with_endpoint(
        &self,
        local_time: Option<f64>,
        playback_rate: f64,
        inclusive_endpoint: bool,
    ) -> EffectTimes {
        let end_time = self.end_time();
        let active_duration = self.active_duration();
        let timing = self.specified_timing_properties.borrow();
        let start_delay = *timing.start_delay;
        let iteration_duration = timing.iteration_duration.computed_value();
        let iteration_count = timing.iteration_count;
        let iteration_start = *timing.iteration_start;

        let Some(local_time) = local_time else {
            return EffectTimes::default();
        };
        let not_in_effect = |phase| EffectTimes {
            phase: Some(phase),
            ..Default::default()
        };

        // <https://drafts.csswg.org/web-animations-1/#animation-effect-phases-and-states>
        let before_active_boundary_time = start_delay.min(end_time).max(0.0);
        let active_after_boundary_time = (start_delay + active_duration).min(end_time).max(0.0);
        let backwards = playback_rate < 0.0;
        let phase = if local_time < before_active_boundary_time ||
            (backwards && !inclusive_endpoint && local_time == before_active_boundary_time)
        {
            EffectPhase::Before
        } else if local_time > active_after_boundary_time ||
            (!backwards && !inclusive_endpoint && local_time == active_after_boundary_time)
        {
            EffectPhase::After
        } else {
            EffectPhase::Active
        };

        // <https://drafts.csswg.org/web-animations-1/#calculating-the-active-time>
        // > fill mode auto: For keyframe effects, behave as if none.
        let fill_mode = timing.fill_mode;
        let fills_backwards = matches!(fill_mode, FillMode::Backwards | FillMode::Both);
        let fills_forwards = matches!(fill_mode, FillMode::Forwards | FillMode::Both);
        let active_time = match phase {
            EffectPhase::Before => fills_backwards.then(|| (local_time - start_delay).max(0.0)),
            EffectPhase::Active => Some(local_time - start_delay),
            EffectPhase::After => {
                fills_forwards.then(|| (local_time - start_delay).min(active_duration).max(0.0))
            },
        };
        let Some(active_time) = active_time else {
            return not_in_effect(phase);
        };

        // <https://drafts.csswg.org/web-animations-1/#calculating-the-overall-progress>
        let overall_progress = if iteration_duration == 0.0 {
            if phase == EffectPhase::Before {
                iteration_start
            } else {
                iteration_start + iteration_count
            }
        } else {
            active_time / iteration_duration + iteration_start
        };

        // <https://drafts.csswg.org/web-animations-1/#calculating-the-simple-iteration-progress>
        let mut simple_iteration_progress = if overall_progress.is_infinite() {
            iteration_start % 1.0
        } else {
            overall_progress % 1.0
        };
        if simple_iteration_progress == 0.0 &&
            matches!(phase, EffectPhase::Active | EffectPhase::After) &&
            active_time == active_duration &&
            iteration_count != 0.0
        {
            simple_iteration_progress = 1.0;
        }

        // <https://drafts.csswg.org/web-animations-1/#calculating-the-current-iteration>
        let current_iteration = if phase == EffectPhase::After && iteration_count.is_infinite() {
            f64::INFINITY
        } else if simple_iteration_progress == 1.0 {
            overall_progress.floor() - 1.0
        } else {
            overall_progress.floor()
        };

        // <https://drafts.csswg.org/web-animations-1/#calculating-the-directed-progress>
        let forwards = match timing.playback_direction {
            PlaybackDirection::Normal => true,
            PlaybackDirection::Reverse => false,
            PlaybackDirection::Alternate | PlaybackDirection::Alternate_reverse => {
                let mut d = current_iteration;
                if timing.playback_direction == PlaybackDirection::Alternate_reverse {
                    d += 1.0;
                }
                d.is_infinite() || d % 2.0 == 0.0
            },
        };
        let directed_progress = if forwards {
            simple_iteration_progress
        } else {
            1.0 - simple_iteration_progress
        };

        // <https://drafts.csswg.org/web-animations-1/#calculating-the-transformed-progress>
        let before_flag = (phase == EffectPhase::Before && forwards) ||
            (phase == EffectPhase::After && !forwards);
        let transformed_progress = timing
            .easing_function
            .to_computed_value_without_context()
            .calculate_output(
                directed_progress,
                if before_flag {
                    BeforeFlag::Set
                } else {
                    BeforeFlag::Unset
                },
                1e-7,
            );

        EffectTimes {
            active_time: Some(active_time),
            current_iteration: Some(current_iteration),
            iteration_progress: Some(transformed_progress),
            before_flag,
            phase: Some(phase),
        }
    }

    /// <https://drafts.csswg.org/web-animations-1/#update-the-timing-properties-of-an-animation-effect>
    pub(crate) fn update_the_timing_properties(
        &self,
        input: &OptionalEffectTiming,
    ) -> Fallible<()> {
        // Step 1. If the iterationStart member of input exists and is less than zero,
        // throw a TypeError and abort this procedure.
        if input
            .iterationStart
            .is_some_and(|iteration_start| *iteration_start < 0.0)
        {
            return Err(Error::Type(
                c"Negative values for iterationStart are not allowed".to_owned(),
            ));
        }

        // Step 2. If the iterations member of input exists, and is less than zero or is the value NaN,
        // throw a TypeError and abort this procedure.
        if input
            .iterations
            .is_some_and(|iterations| iterations < 0.0 || iterations.is_nan())
        {
            return Err(Error::Type(
                c"\"iterations\" must be a positive number".to_owned(),
            ));
        }

        // Step 3. If the duration member of input exists, and is less than zero, is the value NaN,
        // or is a string other than the lowercase auto, throw a TypeError and abort this procedure.
        let Ok(duration) = input
            .duration
            .as_ref()
            .map(|duration| match duration {
                UnrestrictedDoubleOrString::UnrestrictedDouble(double) => {
                    if *double < 0.0 || double.is_nan() {
                        Err(())
                    } else {
                        Ok(IterationDurationOrAuto::Duration(*double))
                    }
                },
                UnrestrictedDoubleOrString::String(string) => {
                    if string == "auto" {
                        Ok(IterationDurationOrAuto::Auto)
                    } else {
                        Err(())
                    }
                },
            })
            .transpose()
        else {
            return Err(Error::Type(
                c"\"duration\" must be a positive number".to_owned(),
            ));
        };

        // Step 4. If the easing member of input exists but cannot be parsed using the <easing-function> production [CSS-EASING-1],
        // throw a TypeError and abort this procedure.
        let Ok(easing) = input
            .easing
            .as_ref()
            .map(|easing| {
                let easing = easing.str();
                let mut parser = Parser::new(&easing);

                // None of these values should matter
                let document = self.window.Document();
                let urlextradata = document.url().into_url().into();
                let parser_context = parser_context_for_document(
                    &document,
                    CssRuleType::Style,
                    ParsingMode::DEFAULT,
                    &urlextradata,
                );
                TimingFunction::parse(&parser_context, &mut parser).map_err(|_| ())
            })
            .transpose()
        else {
            return Err(Error::Type(
                c"\"easing\" is not a valid timing function".to_owned(),
            ));
        };

        // Step 5. Assign each member that exists in input to the corresponding timing property of effect as follows:
        // delay → start delay
        let mut specified_timing_properties = self.specified_timing_properties.borrow_mut();
        if let Some(start_delay) = input.delay {
            specified_timing_properties.start_delay = start_delay;
        }

        // endDelay → end delay
        if let Some(end_delay) = input.endDelay {
            specified_timing_properties.end_delay = end_delay;
        }

        // fill → fill mode
        if let Some(fill) = input.fill {
            specified_timing_properties.fill_mode = fill;
        }

        // iterationStart → iteration start
        if let Some(iteration_start) = input.iterationStart {
            specified_timing_properties.iteration_start = iteration_start;
        }

        // iterations → iteration count
        if let Some(iterations) = input.iterations {
            specified_timing_properties.iteration_count = iterations;
        }

        // duration → iteration duration
        if let Some(duration) = duration {
            specified_timing_properties.iteration_duration = duration;
        }

        // direction → playback direction
        if let Some(direction) = input.direction {
            specified_timing_properties.playback_direction = direction;
        }

        // easing → easing function
        if let Some(easing) = easing {
            specified_timing_properties.easing_function = easing;
        }
        Ok(())
    }
}

impl AnimationEffectMethods<crate::DomTypeHolder> for AnimationEffect {
    /// <https://drafts.csswg.org/web-animations-1/#dom-animationeffect-gettiming>
    fn GetTiming(&self) -> EffectTiming {
        // > Returns the specified timing properties for this animation effect.
        let specified_timing_properties = self.specified_timing_properties.borrow();
        EffectTiming {
            delay: specified_timing_properties.start_delay,
            direction: specified_timing_properties.playback_direction,
            duration: specified_timing_properties.iteration_duration.into(),
            easing: specified_timing_properties
                .easing_function
                .to_css_string()
                .into(),
            endDelay: specified_timing_properties.end_delay,
            fill: specified_timing_properties.fill_mode,
            iterationStart: specified_timing_properties.iteration_start,
            iterations: specified_timing_properties.iteration_count,
        }
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animationeffect-getcomputedtiming>
    fn GetComputedTiming(&self) -> ComputedEffectTiming {
        // > Returns the calculated timing properties for this animation effect.
        // > localTime: The local time of the animation effect, its associated
        // > animation's current time, or null if it has none.
        let animation = self.associated_animation.get();
        let local_time = animation
            .as_ref()
            .and_then(|animation| animation.current_time());
        let playback_rate = animation
            .as_ref()
            .map_or(1.0, |animation| animation.playback_rate());
        let times = self.compute_timing(local_time, playback_rate);
        let end_time = self.end_time();
        let active_duration = self.active_duration();
        let specified_timing_properties = self.specified_timing_properties.borrow();

        // > while getTiming() can return the string auto, getComputedTiming() must return a number
        // > corresponding to the calculated value of the iteration duration as defined in the description
        // > of the duration member of the EffectTiming interface.
        // > In this level of the specification, that simply means that an auto value is replaced by zero.
        let computed_duration = specified_timing_properties
            .iteration_duration
            .computed_value();

        // > likewise, while getTiming() can return the string auto, getComputedTiming() must return the
        // > specific FillMode used for timing calculations as defined in the description of the fill
        // > member of the EffectTiming interface.
        // > In this level of the specification, that simply means that an auto value is replaced by the none FillMode.
        let computed_fill_mode = if specified_timing_properties.fill_mode == FillMode::Auto {
            FillMode::None
        } else {
            specified_timing_properties.fill_mode
        };

        ComputedEffectTiming {
            parent: EffectTiming {
                delay: specified_timing_properties.start_delay,
                direction: specified_timing_properties.playback_direction,
                duration: UnrestrictedDoubleOrString::UnrestrictedDouble(computed_duration),
                easing: specified_timing_properties
                    .easing_function
                    .to_css_string()
                    .into(),
                endDelay: specified_timing_properties.end_delay,
                fill: computed_fill_mode,
                iterationStart: specified_timing_properties.iteration_start,
                iterations: specified_timing_properties.iteration_count,
            },
            endTime: Some(end_time),
            activeDuration: Some(active_duration),
            localTime: Some(local_time.map(Finite::wrap)),
            progress: Some(times.iteration_progress.map(Finite::wrap)),
            currentIteration: Some(times.current_iteration),
        }
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animationeffect-updatetiming>
    fn UpdateTiming(&self, cx: &mut JSContext, timing: &OptionalEffectTiming) -> Fallible<()> {
        // > Updates the specified timing properties of this animation effect by performing the procedure
        // > to update the timing properties of an animation effect passing the timing parameter as input.
        self.update_the_timing_properties(timing)?;
        if let Some(animation) = self.associated_animation.get() {
            // > After a successful call to updateTiming() on the effect of a CSSAnimation,
            // > changes to the corresponding animation-* properties are not reflected.
            let mut flags = 0;
            if timing.duration.is_some() {
                flags |= overridden::DURATION;
            }
            if timing.delay.is_some() {
                flags |= overridden::DELAY;
            }
            if timing.iterations.is_some() {
                flags |= overridden::ITERATIONS;
            }
            if timing.direction.is_some() {
                flags |= overridden::DIRECTION;
            }
            if timing.fill.is_some() {
                flags |= overridden::FILL;
            }
            animation.override_css_properties(flags);
            animation.effect_timing_changed(cx);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, JSTraceable, MallocSizeOf)]
enum IterationDurationOrAuto {
    Duration(f64),
    Auto,
}

impl IterationDurationOrAuto {
    fn computed_value(&self) -> f64 {
        match self {
            IterationDurationOrAuto::Auto => 0.0,
            IterationDurationOrAuto::Duration(double) => *double,
        }
    }
}

impl From<IterationDurationOrAuto> for UnrestrictedDoubleOrString {
    fn from(value: IterationDurationOrAuto) -> Self {
        match value {
            IterationDurationOrAuto::Auto => {
                UnrestrictedDoubleOrString::String(DOMString::from_static("auto"))
            },
            IterationDurationOrAuto::Duration(double) => {
                UnrestrictedDoubleOrString::UnrestrictedDouble(double)
            },
        }
    }
}
