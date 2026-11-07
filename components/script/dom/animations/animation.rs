/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::cmp::Ordering as CmpOrdering;
use std::sync::atomic::{AtomicU64, Ordering};

use cssparser::ToCss;
use dom_struct::dom_struct;
use js::context::{JSContext, NoGC};
use js::rust::HandleObject;
use layout_api::QueryMsg;
use script_bindings::cell::DomRefCell;
use script_bindings::codegen::GenericBindings::AnimationBinding::AnimationPlayState;
use script_bindings::codegen::GenericBindings::DocumentBinding::DocumentMethods;
use script_bindings::codegen::GenericBindings::KeyframeEffectBinding::CompositeOperationOrAuto;
use script_bindings::codegen::GenericBindings::WindowBinding::WindowMethods;
use script_bindings::error::{Error, ErrorResult};
use script_bindings::inheritance::Castable;
use script_bindings::num::Finite;
use script_bindings::reflector::reflect_dom_object_with_proto;
use script_bindings::root::DomRoot;
use script_bindings::str::DOMString;
use style::animation::{AnimationSetKey, AnimationState, ScriptAnimation, ScriptTiming};
use style::properties::animated_properties::AnimationValue;
use style::properties::longhands::animation_composition::computed_value::single_value::T as AnimationComposition;
use style::selector_parser::PseudoElement;
use style::values::specified::TimingFunction as SpecifiedTimingFunction;
use stylo_atoms::Atom;

use crate::dom::animationeffect::{AnimationEffect, CssEffectTiming, EffectPhase, overridden};
use crate::dom::bindings::codegen::Bindings::AnimationBinding::AnimationMethods;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::MutNullableDom;
use crate::dom::cssanimation::CSSAnimation;
use crate::dom::csstransition::CSSTransition;
use crate::dom::document::animation_manager::TransitionOrAnimationEventType;
use crate::dom::element::Element;
use crate::dom::eventtarget::EventTarget;
use crate::dom::globalscope::GlobalScope;
use crate::dom::keyframeeffect::{KeyframeEffect, animation_property_name_to_idl_attribute_name};
use crate::dom::node::{Node, NodeDamage, NodeFlags};
use crate::dom::promise::{Promise, RootedPromise, TracedPromise};
use crate::dom::types::AnimationTimeline;
use crate::dom::window::Window;

/// The source of the identifiers that order animations created by script in the
/// composite order.
static NEXT_ANIMATION_ID: AtomicU64 = AtomicU64::new(1);

/// The values last given to style for an animation, used to avoid restyling its target
/// when nothing changed.
#[derive(Clone, Copy, Default, JSTraceable, MallocSizeOf, PartialEq)]
struct StyleSample {
    iteration_progress: Option<f64>,
    before_flag: bool,
    needs_ticks: bool,
}

/// <https://drafts.csswg.org/web-animations-1/#animation>
#[dom_struct]
pub(crate) struct Animation {
    event_target: EventTarget,

    /// An identifier that orders this animation in the composite order and identifies it
    /// to style.
    unique_id: u64,

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-id>
    id: DomRefCell<DOMString>,

    /// <https://drafts.csswg.org/web-animations-1/#timeline>
    timeline: MutNullableDom<AnimationTimeline>,

    /// <https://drafts.csswg.org/web-animations-1/#animation-associated-effect>
    associated_effect: MutNullableDom<AnimationEffect>,

    /// <https://drafts.csswg.org/web-animations-1/#animation-start-time>
    start_time: Cell<Option<f64>>,

    /// <https://drafts.csswg.org/web-animations-1/#animation-hold-time>
    hold_time: Cell<Option<f64>>,

    /// <https://drafts.csswg.org/web-animations-1/#previous-current-time>
    previous_current_time: Cell<Option<f64>>,

    /// <https://drafts.csswg.org/web-animations-1/#playback-rate>
    playback_rate: Cell<f64>,

    /// <https://drafts.csswg.org/web-animations-1/#pending-playback-rate>
    pending_playback_rate: Cell<Option<f64>>,

    /// <https://drafts.csswg.org/web-animations-1/#pending-play-task>
    pending_play_task: Cell<bool>,

    /// <https://drafts.csswg.org/web-animations-1/#pending-pause-task>
    pending_pause_task: Cell<bool>,

    /// Whether the finish notification steps should run at the next animation update.
    pending_finish_notification: Cell<bool>,

    /// <https://drafts.csswg.org/web-animations-1/#current-ready-promise>
    ready_promise: DomRefCell<Option<TracedPromise>>,

    /// <https://drafts.csswg.org/web-animations-1/#current-finished-promise>
    finished_promise: DomRefCell<Option<TracedPromise>>,

    /// Whether this animation is in the list of animations that the document updates.
    registered: Cell<bool>,

    /// The element that style currently applies this animation to.
    style_target: MutNullableDom<Element>,

    /// The values last given to style for this animation.
    last_style_sample: Cell<Option<StyleSample>>,

    /// For a CSS animation or transition, the state that links it to style.
    css: DomRefCell<Option<CssState>>,

    /// <https://drafts.csswg.org/css-animations-2/#owning-element-section>
    /// <https://drafts.csswg.org/css-transitions-2/#owning-element>
    owning_element: MutNullableDom<Element>,

    /// The effect that style created for a CSS animation or transition.
    css_effect: MutNullableDom<KeyframeEffect>,
}

/// The phase of a CSS animation or transition, used to dispatch its events.
///
/// <https://drafts.csswg.org/css-animations-2/#event-dispatch>
/// <https://drafts.csswg.org/css-transitions-2/#event-dispatch>
#[derive(Clone, Copy, Debug, JSTraceable, MallocSizeOf, PartialEq)]
enum CssPhase {
    Idle,
    Pending,
    Before,
    Active,
    After,
}

/// The state of a CSS animation or transition that links it to the animation or
/// transition that style generated.
#[derive(JSTraceable, MallocSizeOf)]
struct CssState {
    is_transition: bool,
    /// The key of the owning element in the animation sets of style.
    #[no_trace]
    #[ignore_malloc_size_of = "Defined in style"]
    key: AnimationSetKey,
    /// The identifier of the animation or transition in the animation sets of style.
    style_id: u64,
    /// The animation name, or the expanded transition property name.
    name: String,
    /// For an animation, the position of its name in `animation-name`. For a transition,
    /// its transition generation.
    order: u64,
    /// The [`overridden`] flags for the properties that script changed.
    overridden: u8,
    /// Whether `animation-play-state` was `paused` at the last style update.
    style_paused: bool,
    /// The timing properties from the last style update.
    style_timing: CssEffectTiming,
    previous_phase: CssPhase,
    previous_iteration: Option<f64>,
    /// The local time when the events were last determined.
    previous_local_time: Option<f64>,
}

impl Animation {
    pub(crate) fn new_inherited() -> Self {
        Self {
            event_target: EventTarget::new_inherited(),
            unique_id: NEXT_ANIMATION_ID.fetch_add(1, Ordering::Relaxed),
            id: Default::default(),
            timeline: Default::default(),
            associated_effect: Default::default(),
            start_time: Default::default(),
            hold_time: Default::default(),
            previous_current_time: Default::default(),
            playback_rate: Cell::new(1.0),
            pending_playback_rate: Default::default(),
            pending_play_task: Default::default(),
            pending_pause_task: Default::default(),
            pending_finish_notification: Default::default(),
            ready_promise: Default::default(),
            finished_promise: Default::default(),
            registered: Default::default(),
            style_target: Default::default(),
            last_style_sample: Default::default(),
            css: Default::default(),
            owning_element: Default::default(),
            css_effect: Default::default(),
        }
    }

    fn new_with_proto(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
    ) -> DomRoot<Self> {
        let animation =
            reflect_dom_object_with_proto(cx, Box::new(Self::new_inherited()), global, proto);
        animation.initialize_promises(cx);
        animation
    }

    /// Create the initial ready and finished promises of a new animation.
    pub(crate) fn initialize_promises(&self, cx: &mut JSContext) {
        let global = self.global();
        // > The current ready promise is initially a resolved Promise created using the
        // > procedure to create a new resolved Promise with the animation itself as its value.
        let ready_promise = Promise::new_resolved(cx, &global, DomRoot::from_ref(self));
        *self.ready_promise.borrow_mut() = Some(ready_promise.to_traced());

        // > The current finished promise is initially a pending Promise object.
        let finished_promise = Promise::new(cx, &global);
        *self.finished_promise.borrow_mut() = Some(finished_promise.to_traced());
    }

    fn ready_promise(&self, cx: &JSContext) -> RootedPromise {
        self.ready_promise
            .borrow()
            .as_ref()
            .expect("Animation should always have a ready promise")
            .root(cx)
    }

    fn finished_promise(&self, cx: &JSContext) -> RootedPromise {
        self.finished_promise
            .borrow()
            .as_ref()
            .expect("Animation should always have a finished promise")
            .root(cx)
    }

    fn replace_ready_promise(&self, cx: &mut JSContext) {
        let promise = Promise::new(cx, &self.global());
        *self.ready_promise.borrow_mut() = Some(promise.to_traced());
    }

    fn resolve_ready_promise(&self, cx: &mut JSContext) {
        let promise = self.ready_promise(cx);
        if promise.is_pending() {
            promise.resolve_native(cx, &DomRoot::from_ref(self));
        }
    }

    /// The current time of the timeline of this animation, if it has one and it is active.
    fn timeline_time(&self) -> Option<f64> {
        self.timeline
            .get()
            .map(|timeline| timeline.current_time_in_seconds() * 1000.0)
    }

    /// <https://drafts.csswg.org/web-animations-1/#animation-current-time>
    pub(crate) fn current_time(&self) -> Option<f64> {
        self.current_time_with_hold_time(self.hold_time.get())
    }

    fn current_time_with_hold_time(&self, hold_time: Option<f64>) -> Option<f64> {
        // > If the animation’s hold time is resolved, the current time is the animation’s hold time.
        if hold_time.is_some() {
            return hold_time;
        }
        // > If any of the following are true: the animation has no associated timeline, or
        // > the associated timeline is inactive, or the animation’s start time is unresolved;
        // > the current time is an unresolved time value.
        let timeline_time = self.timeline_time()?;
        let start_time = self.start_time.get()?;
        // > Otherwise, current time = (timeline time - start time) × playback rate
        Some((timeline_time - start_time) * self.playback_rate.get())
    }

    pub(crate) fn playback_rate(&self) -> f64 {
        self.playback_rate.get()
    }

    /// <https://drafts.csswg.org/web-animations-1/#effective-playback-rate>
    fn effective_playback_rate(&self) -> f64 {
        self.pending_playback_rate
            .get()
            .unwrap_or(self.playback_rate.get())
    }

    /// <https://drafts.csswg.org/web-animations-1/#apply-any-pending-playback-rate>
    fn apply_any_pending_playback_rate(&self) {
        if let Some(rate) = self.pending_playback_rate.take() {
            self.playback_rate.set(rate);
        }
    }

    /// <https://drafts.csswg.org/web-animations-1/#associated-effect-end>
    fn associated_effect_end(&self) -> f64 {
        self.associated_effect
            .get()
            .map_or(0.0, |effect| effect.end_time())
    }

    /// <https://drafts.csswg.org/web-animations-1/#pending>
    fn is_pending(&self) -> bool {
        self.pending_play_task.get() || self.pending_pause_task.get()
    }

    /// <https://drafts.csswg.org/web-animations-1/#play-state>
    fn play_state(&self) -> AnimationPlayState {
        let current_time = self.current_time();
        // > idle: The current time of animation is unresolved and the start time of animation
        // > is unresolved and animation does not have either a pending play task or a pending
        // > pause task.
        if current_time.is_none() && self.start_time.get().is_none() && !self.is_pending() {
            return AnimationPlayState::Idle;
        }
        // > paused: Either of the following conditions are true: animation has a pending pause
        // > task, or both the start time of animation is unresolved and it does not have a
        // > pending play task.
        if self.pending_pause_task.get() ||
            (self.start_time.get().is_none() && !self.pending_play_task.get())
        {
            return AnimationPlayState::Paused;
        }
        // > finished: For animation, current time is resolved and either of the following
        // > conditions are true: animation’s effective playback rate > 0 and current time ≥
        // > associated effect end; or animation’s effective playback rate < 0 and current
        // > time ≤ 0.
        if let Some(current_time) = current_time {
            let rate = self.effective_playback_rate();
            if (rate > 0.0 && current_time >= self.associated_effect_end()) ||
                (rate < 0.0 && current_time <= 0.0)
            {
                return AnimationPlayState::Finished;
            }
        }
        AnimationPlayState::Running
    }

    /// <https://drafts.csswg.org/web-animations-1/#animation-set-the-timeline-of-an-animation>
    fn set_the_timeline(&self, cx: &mut JSContext, timeline: Option<&AnimationTimeline>) {
        // Step 1. Let old timeline be the current timeline of animation, if any.
        // Step 2. If new timeline is the same object as old timeline, abort this procedure.
        if self.timeline.get().as_deref() == timeline {
            return;
        }
        // Remember the local time of a CSS animation or transition, for the elapsed time of
        // its cancel event if the new timeline makes it idle.
        let current_time = self.current_time();
        if let Some(css) = self.css.borrow_mut().as_mut() {
            css.previous_local_time = current_time;
        }
        // Step 3. Let the timeline of animation be new timeline.
        self.timeline.set(timeline);
        // Step 4. If the start time of animation is resolved, make animation’s hold time
        // unresolved.
        if self.start_time.get().is_some() {
            self.hold_time.set(None);
        }
        // Step 5. Run the procedure to update an animation’s finished state for animation
        // with the did seek flag set to false, and the synchronously notify flag set to false.
        self.update_finished_state(cx, false, false);
        self.update_style(cx);
    }

    /// <https://drafts.csswg.org/web-animations-1/#animation-set-the-associated-effect-of-an-animation>
    fn set_the_associated_effect(&self, cx: &mut JSContext, effect: Option<&AnimationEffect>) {
        // Step 1. Let old effect be the current associated effect of animation, if any.
        let old_effect = self.associated_effect.get();
        // Step 2. If new effect is the same object as old effect, abort this procedure.
        if old_effect.as_deref() == effect {
            return;
        }
        // Step 3. If animation has a pending pause task, reschedule that task to run as soon
        // as animation is ready.
        // Step 4. If animation has a pending play task, reschedule that task to run as soon
        // as animation is ready to play new effect.
        // Note: Pending tasks run at the next animation update in any case.

        // Step 5. If new effect is not null and if new effect is the associated effect of
        // another animation, previous animation, run the procedure to set the associated
        // effect of an animation (this procedure) on previous animation passing null as new
        // effect.
        if let Some(effect) = effect {
            if let Some(previous_animation) = effect.associated_animation() {
                if &*previous_animation != self {
                    previous_animation.set_the_associated_effect(cx, None);
                }
            }
        }
        if let Some(old_effect) = old_effect {
            old_effect.set_associated_animation(None);
        }

        // Step 6. Let the associated effect of animation be new effect.
        self.associated_effect.set(effect);
        if let Some(effect) = effect {
            effect.set_associated_animation(Some(self));
        }

        // Step 7. Run the procedure to update an animation’s finished state for animation
        // with the did seek flag set to false, and the synchronously notify flag set to false.
        self.update_finished_state(cx, false, false);
        self.update_style(cx);
    }

    /// <https://drafts.csswg.org/web-animations-1/#silently-set-the-current-time>
    fn silently_set_the_current_time(&self, seek_time: Option<f64>) -> ErrorResult {
        // Step 1. If seek time is an unresolved time value, then perform the following steps.
        let Some(seek_time) = seek_time else {
            // Step 1.1. If the current time is resolved, then throw a TypeError.
            if self.current_time().is_some() {
                return Err(Error::Type(
                    c"Cannot set the current time of a playing animation to null".to_owned(),
                ));
            }
            // Step 1.2. Abort these steps.
            return Ok(());
        };

        // Step 2. Update either animation’s hold time or start time as follows:
        // > If any of the following conditions are true: animation’s hold time is resolved,
        // > or animation’s start time is unresolved, or animation has no associated timeline
        // > or the associated timeline is inactive, or animation’s playback rate is 0,
        // > Set animation’s hold time to seek time.
        let timeline_time = self.timeline_time();
        if self.hold_time.get().is_some() ||
            self.start_time.get().is_none() ||
            timeline_time.is_none() ||
            self.playback_rate.get() == 0.0
        {
            self.hold_time.set(Some(seek_time));
        } else if let Some(timeline_time) = timeline_time {
            // > Otherwise, Set animation’s start time to the result of evaluating
            // > timeline time − (seek time / playback rate)
            self.start_time
                .set(Some(timeline_time - seek_time / self.playback_rate.get()));
        }

        // Step 3. If animation has no associated timeline or the associated timeline is
        // inactive, make animation’s start time unresolved.
        if timeline_time.is_none() {
            self.start_time.set(None);
        }

        // Step 4. Make animation’s previous current time unresolved.
        self.previous_current_time.set(None);
        Ok(())
    }

    /// <https://drafts.csswg.org/web-animations-1/#set-the-current-time>
    fn set_the_current_time(&self, cx: &mut JSContext, seek_time: Option<f64>) -> ErrorResult {
        // Step 1. Run the steps to silently set the current time of animation to seek time.
        self.silently_set_the_current_time(seek_time)?;

        // Step 2. If animation has a pending pause task, synchronously complete the pause
        // operation by performing the following steps:
        if self.pending_pause_task.get() {
            // Step 2.1. Set animation’s hold time to seek time.
            self.hold_time.set(seek_time);
            // Step 2.2. Apply any pending playback rate to animation.
            self.apply_any_pending_playback_rate();
            // Step 2.3. Make animation’s start time unresolved.
            self.start_time.set(None);
            // Step 2.4. Cancel the pending pause task.
            self.pending_pause_task.set(false);
            // Step 2.5. Resolve animation’s current ready promise with animation.
            self.resolve_ready_promise(cx);
        }

        // Step 3. Run the procedure to update an animation’s finished state for animation
        // with the did seek flag set to true, and the synchronously notify flag set to false.
        self.update_finished_state(cx, true, false);
        self.update_style(cx);
        Ok(())
    }

    /// <https://drafts.csswg.org/web-animations-1/#set-the-start-time>
    fn set_the_start_time(&self, cx: &mut JSContext, new_start_time: Option<f64>) {
        // Step 1. Let timeline time be the current time value of the timeline that animation
        // is associated with. If there is no timeline associated with animation or the
        // associated timeline is inactive, let the timeline time be unresolved.
        let timeline_time = self.timeline_time();

        // Step 2. If timeline time is unresolved and new start time is resolved, make
        // animation’s hold time unresolved.
        if timeline_time.is_none() && new_start_time.is_some() {
            self.hold_time.set(None);
        }

        // Step 3. Let previous current time be animation’s current time.
        let previous_current_time = self.current_time();

        // Step 4. Apply any pending playback rate on animation.
        self.apply_any_pending_playback_rate();

        // Step 5. Set animation’s start time to new start time.
        self.start_time.set(new_start_time);

        // Step 6. Update animation’s hold time based on the first matching condition from
        // the following.
        if new_start_time.is_some() {
            // > If new start time is resolved, If animation’s playback rate is not zero, make
            // > animation’s hold time unresolved.
            if self.playback_rate.get() != 0.0 {
                self.hold_time.set(None);
            }
        } else {
            // > Otherwise (new start time is unresolved), Set animation’s hold time to
            // > previous current time even if previous current time is unresolved.
            self.hold_time.set(previous_current_time);
        }

        // Step 7. If animation has a pending play task or a pending pause task, cancel that
        // task and resolve animation’s current ready promise with animation.
        if self.is_pending() {
            self.pending_play_task.set(false);
            self.pending_pause_task.set(false);
            self.resolve_ready_promise(cx);
        }

        // Step 8. Run the procedure to update an animation’s finished state for animation
        // with the did seek flag set to true, and the synchronously notify flag set to false.
        self.update_finished_state(cx, true, false);
        self.update_style(cx);
    }

    /// <https://drafts.csswg.org/web-animations-1/#play-an-animation>
    fn play_an_animation(&self, cx: &mut JSContext, auto_rewind: bool) -> ErrorResult {
        // Step 1. Let aborted pause be a boolean flag that is true if animation has a pending
        // pause task, and false otherwise.
        let aborted_pause = self.pending_pause_task.get();

        // Step 2. Let has pending ready promise be a boolean flag that is initially false.
        let mut has_pending_ready_promise = false;

        // Step 3. Let seek time be a time value that is initially unresolved.
        let mut seek_time = None;

        // Step 4. If the auto-rewind flag is true, perform the steps corresponding to the
        // first matching condition from the following, if any:
        let effective_playback_rate = self.effective_playback_rate();
        let current_time = self.current_time();
        let associated_effect_end = self.associated_effect_end();
        if auto_rewind {
            if effective_playback_rate >= 0.0 &&
                current_time.is_none_or(|current_time| {
                    current_time < 0.0 || current_time >= associated_effect_end
                })
            {
                // > If animation’s effective playback rate ≥ 0, and animation’s current time is
                // > either: unresolved, or less than zero, or greater than or equal to associated
                // > effect end, Set seek time to zero.
                seek_time = Some(0.0);
            } else if effective_playback_rate < 0.0 &&
                current_time.is_none_or(|current_time| {
                    current_time <= 0.0 || current_time > associated_effect_end
                })
            {
                // > If animation’s effective playback rate < 0, and animation’s current time is
                // > either: unresolved, or less than or equal to zero, or greater than
                // > associated effect end,
                // > If associated effect end is positive infinity, throw an "InvalidStateError"
                // > DOMException and abort these steps.
                if associated_effect_end == f64::INFINITY {
                    return Err(Error::InvalidState(Some(
                        "Cannot play a reversed animation with an infinite end".to_owned(),
                    )));
                }
                // > Otherwise, Set seek time to animation’s associated effect end.
                seek_time = Some(associated_effect_end);
            }
        }

        // Step 5. If the following three conditions are all satisfied: seek time is
        // unresolved, and animation’s effective playback rate = 0, and animation’s current
        // time is unresolved, set seek time to zero.
        if seek_time.is_none() && effective_playback_rate == 0.0 && current_time.is_none() {
            seek_time = Some(0.0);
        }

        // Step 6. If seek time is resolved, set animation’s hold time to seek time.
        if seek_time.is_some() {
            self.hold_time.set(seek_time);
        }

        // Step 7. If animation’s hold time is resolved, let its start time be unresolved.
        if self.hold_time.get().is_some() {
            self.start_time.set(None);
        }

        // Step 8. If animation has a pending play task or a pending pause task, cancel that
        // task and set has pending ready promise to true.
        if self.is_pending() {
            self.pending_play_task.set(false);
            self.pending_pause_task.set(false);
            has_pending_ready_promise = true;
        }

        // Step 9. If the following four conditions are all satisfied: animation’s hold time is
        // unresolved, and seek time is unresolved, and aborted pause is false, and animation
        // does not have a pending playback rate, abort this procedure.
        if self.hold_time.get().is_none() &&
            seek_time.is_none() &&
            !aborted_pause &&
            self.pending_playback_rate.get().is_none()
        {
            return Ok(());
        }

        // Step 10. If has pending ready promise is false, let animation’s current ready promise
        // be a new promise in the relevant Realm of animation.
        if !has_pending_ready_promise {
            self.replace_ready_promise(cx);
        }

        // Step 11. Schedule a task to run as soon as animation is ready.
        self.pending_play_task.set(true);
        self.register_with_document();

        // Step 12. Run the procedure to update an animation’s finished state for animation
        // with the did seek flag set to false, and the synchronously notify flag set to false.
        self.update_finished_state(cx, false, false);
        self.update_style(cx);
        Ok(())
    }

    /// The pending play task of <https://drafts.csswg.org/web-animations-1/#play-an-animation>.
    fn run_pending_play_task(&self, cx: &mut JSContext, ready_time: f64) {
        self.pending_play_task.set(false);

        // Step 1. Assert that at least one of animation’s start time or hold time is resolved.
        // Step 2. Let ready time be the time value of the timeline associated with animation
        // at the moment when animation became ready.
        // Step 3. Perform the steps corresponding to the first matching condition below, if any:
        if let Some(hold_time) = self.hold_time.get() {
            // > If animation’s hold time is resolved,
            // > Apply any pending playback rate on animation.
            self.apply_any_pending_playback_rate();
            // > Let new start time be the result of evaluating ready time - hold time /
            // > playback rate for animation. If the playback rate is zero, let new start time
            // > be simply ready time.
            let playback_rate = self.playback_rate.get();
            let new_start_time = if playback_rate == 0.0 {
                ready_time
            } else {
                ready_time - hold_time / playback_rate
            };
            // > Set the start time of animation to new start time.
            self.start_time.set(Some(new_start_time));
            // > If animation’s playback rate is not 0, make animation’s hold time unresolved.
            if playback_rate != 0.0 {
                self.hold_time.set(None);
            }
        } else if let (Some(start_time), Some(pending_playback_rate)) =
            (self.start_time.get(), self.pending_playback_rate.get())
        {
            // > If animation’s start time is resolved and animation has a pending playback rate,
            // > Let current time to match be the result of evaluating (ready time - start
            // > time) × playback rate for animation.
            let current_time_to_match = (ready_time - start_time) * self.playback_rate.get();
            // > Apply any pending playback rate on animation.
            self.apply_any_pending_playback_rate();
            // > If animation’s playback rate is zero, let animation’s hold time be current time
            // > to match.
            if pending_playback_rate == 0.0 {
                self.hold_time.set(Some(current_time_to_match));
            } else {
                // > Let new start time be the result of evaluating ready time - current time to
                // > match / playback rate for animation. If the playback rate is zero, let new
                // > start time be simply ready time.
                // > Set the start time of animation to new start time.
                self.start_time.set(Some(
                    ready_time - current_time_to_match / pending_playback_rate,
                ));
            }
        }

        // Step 4. Resolve animation’s current ready promise with animation.
        self.resolve_ready_promise(cx);

        // Step 5. Run the procedure to update an animation’s finished state for animation with
        // the did seek flag set to false, and the synchronously notify flag set to false.
        self.update_finished_state(cx, false, false);
    }

    /// <https://drafts.csswg.org/web-animations-1/#pause-an-animation>
    fn pause_an_animation(&self, cx: &mut JSContext) -> ErrorResult {
        // Step 1. If animation has a pending pause task, abort these steps.
        // Step 2. If the play state of animation is paused, abort these steps.
        if self.pending_pause_task.get() || self.play_state() == AnimationPlayState::Paused {
            return Ok(());
        }

        // Step 3. Let seek time be a time value that is initially unresolved.
        let mut seek_time = None;

        // Step 4. Let has finite timeline be true if animation has an associated timeline
        // that is not monotonically increasing.
        // Note: Only monotonically increasing timelines are supported.

        // Step 5. If the animation’s current time is unresolved, perform the steps according
        // to the first matching condition below:
        if self.current_time().is_none() {
            if self.playback_rate.get() >= 0.0 {
                // > If animation’s playback rate is ≥ 0, Set seek time to zero.
                seek_time = Some(0.0);
            } else {
                // > Otherwise, If associated effect end for animation is positive infinity,
                // > throw an "InvalidStateError" DOMException and abort these steps.
                let associated_effect_end = self.associated_effect_end();
                if associated_effect_end == f64::INFINITY {
                    return Err(Error::InvalidState(Some(
                        "Cannot pause a reversed animation with an infinite end".to_owned(),
                    )));
                }
                // > Otherwise, let seek time be animation’s associated effect end.
                seek_time = Some(associated_effect_end);
            }
        }

        // Step 6. If seek time is resolved, set animation’s hold time to seek time.
        if seek_time.is_some() {
            self.hold_time.set(seek_time);
        }

        // Step 7. Let has pending ready promise be a boolean flag that is initially false.
        // Step 8. If animation has a pending play task, cancel that task and let has pending
        // ready promise be true.
        let has_pending_ready_promise = self.pending_play_task.replace(false);

        // Step 9. If has pending ready promise is false, set animation’s current ready promise
        // to a new promise in the relevant Realm of animation.
        if !has_pending_ready_promise {
            self.replace_ready_promise(cx);
        }

        // Step 10. Schedule a task to be executed at the first possible moment where both of
        // the following conditions are true: the user agent has performed any processing
        // necessary to suspend the playback of animation’s associated effect, if any; the
        // animation is associated with a timeline that is not inactive.
        self.pending_pause_task.set(true);
        self.register_with_document();

        // Step 11. Run the procedure to update an animation’s finished state for animation
        // with the did seek flag set to false, and the synchronously notify flag set to false.
        self.update_finished_state(cx, false, false);
        self.update_style(cx);
        Ok(())
    }

    /// The pending pause task of <https://drafts.csswg.org/web-animations-1/#pause-an-animation>.
    fn run_pending_pause_task(&self, cx: &mut JSContext, ready_time: f64) {
        self.pending_pause_task.set(false);

        // Step 1. Let ready time be the time value of the timeline associated with animation
        // at the moment when the user agent completed processing necessary to suspend playback
        // of animation’s associated effect.
        // Step 2. If animation’s start time is resolved and its hold time is not resolved, let
        // animation’s hold time be the result of evaluating (ready time - start time) ×
        // playback rate.
        if let (Some(start_time), None) = (self.start_time.get(), self.hold_time.get()) {
            self.hold_time
                .set(Some((ready_time - start_time) * self.playback_rate.get()));
        }

        // Step 3. Apply any pending playback rate on animation.
        self.apply_any_pending_playback_rate();

        // Step 4. Make animation’s start time unresolved.
        self.start_time.set(None);

        // Step 5. Resolve animation’s current ready promise with animation.
        self.resolve_ready_promise(cx);

        // Step 6. Run the procedure to update an animation’s finished state for animation with
        // the did seek flag set to false, and the synchronously notify flag set to false.
        self.update_finished_state(cx, false, false);
    }

    /// <https://drafts.csswg.org/web-animations-1/#update-an-animations-finished-state>
    fn update_finished_state(
        &self,
        cx: &mut JSContext,
        did_seek: bool,
        synchronously_notify: bool,
    ) {
        // Step 1. Let the unconstrained current time be the result of calculating the current
        // time substituting an unresolved time value for the hold time if did seek is false.
        // If did seek is true, the unconstrained current time is equal to the current time.
        let unconstrained_current_time = if did_seek {
            self.current_time()
        } else {
            self.current_time_with_hold_time(None)
        };

        // Step 2. If all three of the following conditions are true, the unconstrained current
        // time is resolved, and animation’s start time is resolved, and animation does not
        // have a pending play task or a pending pause task,
        if let (Some(unconstrained_current_time), Some(_)) =
            (unconstrained_current_time, self.start_time.get())
        {
            if !self.is_pending() {
                let playback_rate = self.playback_rate.get();
                let associated_effect_end = self.associated_effect_end();
                // then update animation’s hold time based on the first matching condition for
                // animation from below, if any:
                if playback_rate > 0.0 && unconstrained_current_time >= associated_effect_end {
                    // > If did seek is true, let the hold time be the value of unconstrained
                    // > current time. If did seek is false, let the hold time be the maximum
                    // > value of previous current time and associated effect end. If the
                    // > previous current time is unresolved, let the hold time be associated
                    // > effect end.
                    self.hold_time.set(Some(if did_seek {
                        unconstrained_current_time
                    } else {
                        self.previous_current_time
                            .get()
                            .map_or(associated_effect_end, |previous| {
                                previous.max(associated_effect_end)
                            })
                    }));
                } else if playback_rate < 0.0 && unconstrained_current_time <= 0.0 {
                    // > If did seek is true, let the hold time be the value of unconstrained
                    // > current time. If did seek is false, let the hold time be the minimum
                    // > value of previous current time and zero. If the previous current time
                    // > is unresolved, let the hold time be zero.
                    self.hold_time.set(Some(if did_seek {
                        unconstrained_current_time
                    } else {
                        self.previous_current_time
                            .get()
                            .map_or(0.0, |previous| previous.min(0.0))
                    }));
                } else if playback_rate != 0.0 {
                    // > If playback rate ≠ 0, and animation is associated with an active
                    // > timeline, Perform the following steps:
                    if let Some(timeline_time) = self.timeline_time() {
                        // > If did seek is true and the hold time is resolved, let animation’s
                        // > start time be equal to the result of evaluating timeline time -
                        // > (hold time / playback rate) where timeline time is the current time
                        // > value of timeline associated with animation.
                        if did_seek {
                            if let Some(hold_time) = self.hold_time.get() {
                                self.start_time
                                    .set(Some(timeline_time - hold_time / playback_rate));
                            }
                        }
                        // > Let the hold time be unresolved.
                        self.hold_time.set(None);
                    }
                }
            }
        }

        // Step 3. Set the previous current time of animation be the result of calculating its
        // current time.
        self.previous_current_time.set(self.current_time());

        // Step 4. Let current finished state be true if the play state of animation is
        // finished. Otherwise, let it be false.
        let current_finished_state = self.play_state() == AnimationPlayState::Finished;

        // Step 5. If current finished state is true and the current finished promise is not
        // yet resolved, perform the following steps:
        let finished_promise = self.finished_promise(cx);
        if current_finished_state && finished_promise.is_pending() {
            // > If synchronously notify is true, cancel any queued microtask to run the finish
            // > notification steps for this animation, and run the finish notification steps
            // > immediately.
            // > Otherwise, if synchronously notify is false, queue a microtask to run finish
            // > notification steps for animation unless there is already a microtask queued to
            // > run those steps for animation.
            if synchronously_notify {
                self.pending_finish_notification.set(false);
                self.run_finish_notification_steps(cx);
            } else {
                self.pending_finish_notification.set(true);
                self.register_with_document();
            }
        }

        // Step 6. If current finished state is false and animation’s current finished promise
        // is already resolved, set animation’s current finished promise to a new promise in the
        // relevant Realm of animation.
        if !current_finished_state && finished_promise.is_fulfilled() {
            let promise = Promise::new(cx, &self.global());
            *self.finished_promise.borrow_mut() = Some(promise.to_traced());
        }
    }

    /// The finish notification steps of
    /// <https://drafts.csswg.org/web-animations-1/#update-an-animations-finished-state>.
    fn run_finish_notification_steps(&self, cx: &mut JSContext) {
        // Step 1. If animation’s play state is not equal to finished, abort these steps.
        if self.play_state() != AnimationPlayState::Finished {
            return;
        }
        // Step 2. Resolve animation’s current finished promise object with animation.
        let finished_promise = self.finished_promise(cx);
        if finished_promise.is_pending() {
            finished_promise.resolve_native(cx, &DomRoot::from_ref(self));
        }
        // Step 3. Create an AnimationPlaybackEvent, finishEvent.
        // Step 4. Set finishEvent’s type attribute to finish.
        // Step 5. Set finishEvent’s currentTime attribute to the current time of animation.
        // Step 6. Set finishEvent’s timelineTime attribute to the current time of the timeline
        // with which animation is associated. If animation is not associated with a timeline,
        // or the timeline is inactive, let timelineTime be null.
        // Step 7. If animation has a document for timing, then append finishEvent to its
        // document for timing's pending animation event queue along with its target, animation.
        self.queue_playback_event(Atom::from("finish"), self.current_time());
    }

    fn queue_playback_event(&self, event_type: Atom, current_time: Option<f64>) {
        let global = self.global();
        let window = global.as_window();
        let document = window.Document();
        let manager = document.animation_manager();
        manager.queue_playback_event(self, event_type, current_time, self.timeline_time());
        manager.handle_animation_presence_or_pending_events_change(window);
    }

    /// <https://drafts.csswg.org/web-animations-1/#reset-an-animations-pending-tasks>
    fn reset_pending_tasks(&self, cx: &mut JSContext) {
        // Step 1. If animation does not have a pending play task or a pending pause task,
        // abort this procedure.
        if !self.is_pending() {
            return;
        }
        // Step 2. If animation has a pending play task, cancel that task.
        // Step 3. If animation has a pending pause task, cancel that task.
        self.pending_play_task.set(false);
        self.pending_pause_task.set(false);
        // Step 4. Apply any pending playback rate on animation.
        self.apply_any_pending_playback_rate();
        // Step 5. Reject animation’s current ready promise with a DOMException named "AbortError".
        // Step 6. Set the [[PromiseIsHandled]] internal slot of animation’s current ready
        // promise to true.
        let ready_promise = self.ready_promise(cx);
        ready_promise.reject_error(
            cx,
            Error::Abort(Some(
                "The pending tasks of the animation were reset".to_owned(),
            )),
        );
        ready_promise.set_promise_is_handled(cx);
        // Step 7. Let animation’s current ready promise be the result of creating a new
        // resolved Promise object with value animation in the relevant Realm of animation.
        let promise = Promise::new_resolved(cx, &self.global(), DomRoot::from_ref(self));
        *self.ready_promise.borrow_mut() = Some(promise.to_traced());
    }

    /// <https://drafts.csswg.org/web-animations-1/#cancel-an-animation>
    fn cancel_an_animation(&self, cx: &mut JSContext) {
        // Step 1. If animation’s play state is not idle, perform the following steps:
        if self.play_state() != AnimationPlayState::Idle {
            self.queue_css_cancel_event();
            // Step 1.1. Run the procedure to reset an animation’s pending tasks on animation.
            self.reset_pending_tasks(cx);
            // Step 1.2. Reject the current finished promise with a DOMException named
            // "AbortError".
            // Step 1.3. Set the [[PromiseIsHandled]] internal slot of the current finished
            // promise to true.
            let finished_promise = self.finished_promise(cx);
            if finished_promise.is_pending() {
                finished_promise.reject_error(
                    cx,
                    Error::Abort(Some("The animation was canceled".to_owned())),
                );
            }
            finished_promise.set_promise_is_handled(cx);
            // Step 1.4. Let current finished promise be a new promise in the relevant Realm of
            // animation.
            let promise = Promise::new(cx, &self.global());
            *self.finished_promise.borrow_mut() = Some(promise.to_traced());
            // Step 1.5. Let cancelEvent be a new AnimationPlaybackEvent with type cancel, a null
            // currentTime and the current time of the timeline as timelineTime, and queue it.
            self.queue_playback_event(atom!("cancel"), None);
        }
        // Step 2. Make animation’s hold time unresolved.
        self.hold_time.set(None);
        // Step 3. Make animation’s start time unresolved.
        self.start_time.set(None);
        self.pending_finish_notification.set(false);
        self.update_style(cx);
    }

    /// <https://drafts.csswg.org/web-animations-1/#finish-an-animation>
    fn finish_an_animation(&self, cx: &mut JSContext) -> ErrorResult {
        // Step 1. If animation’s effective playback rate is zero, or if animation’s effective
        // playback rate > 0 and associated effect end is infinity, throw an
        // "InvalidStateError" DOMException and abort these steps.
        let effective_playback_rate = self.effective_playback_rate();
        let associated_effect_end = self.associated_effect_end();
        if effective_playback_rate == 0.0 ||
            (effective_playback_rate > 0.0 && associated_effect_end == f64::INFINITY)
        {
            return Err(Error::InvalidState(Some(
                "Cannot finish an animation with a zero playback rate or an infinite end"
                    .to_owned(),
            )));
        }

        // Step 2. Apply any pending playback rate to animation.
        self.apply_any_pending_playback_rate();

        // Step 3. Set limit as follows: If playback rate > 0, let limit be associated effect
        // end. Otherwise, let limit be zero.
        let playback_rate = self.playback_rate.get();
        let limit = if playback_rate > 0.0 {
            associated_effect_end
        } else {
            0.0
        };

        // Step 4. Silently set the current time to limit.
        self.silently_set_the_current_time(Some(limit))?;

        // Step 5. If animation’s start time is unresolved and animation has an associated
        // active timeline, let the start time be the result of evaluating timeline time -
        // (limit / playback rate) where timeline time is the current time value of the
        // associated timeline.
        if self.start_time.get().is_none() {
            if let Some(timeline_time) = self.timeline_time() {
                self.start_time
                    .set(Some(timeline_time - limit / playback_rate));
            }
        }

        // Step 6. If there is a pending pause task and start time is resolved,
        if self.pending_pause_task.get() && self.start_time.get().is_some() {
            // Step 6.1. Let the hold time be unresolved.
            self.hold_time.set(None);
            // Step 6.2. Cancel the pending pause task.
            self.pending_pause_task.set(false);
            // Step 6.3. Resolve the current ready promise of animation with animation.
            self.resolve_ready_promise(cx);
        }

        // Step 7. If there is a pending play task and start time is resolved, cancel that task
        // and resolve the current ready promise of animation with animation.
        if self.pending_play_task.get() && self.start_time.get().is_some() {
            self.pending_play_task.set(false);
            self.resolve_ready_promise(cx);
        }

        // Step 8. Run the procedure to update an animation’s finished state for animation with
        // the did seek flag set to true, and the synchronously notify flag set to true.
        self.update_finished_state(cx, true, true);
        self.update_style(cx);
        Ok(())
    }

    /// Add this animation to the list of animations that its document updates at every
    /// animation frame.
    fn register_with_document(&self) {
        if self.registered.replace(true) {
            return;
        }
        let global = self.global();
        global
            .as_window()
            .Document()
            .animation_manager()
            .register_script_animation(self);
    }

    /// Called by the document at every animation update. Runs pending tasks and updates the
    /// style of the target. Returns whether the animation should stay registered.
    pub(crate) fn update_for_animation_frame(&self, cx: &mut JSContext) -> bool {
        let timeline_time = self.timeline_time();
        if let Some(ready_time) = timeline_time {
            if self.pending_play_task.get() {
                self.run_pending_play_task(cx, ready_time);
            } else if self.pending_pause_task.get() {
                self.run_pending_pause_task(cx, ready_time);
            }
        }

        // > Update the current time of all timelines associated with doc passing now as the
        // > timestamp. [..] Updating the current time of a timeline also involves running the
        // > update an animation’s finished state procedure for any animations associated with
        // > the timeline.
        self.update_finished_state(cx, false, false);
        if self.pending_finish_notification.replace(false) {
            self.run_finish_notification_steps(cx);
        }
        self.update_style(cx);
        self.update_css_events();

        // Stop updating animations that cannot change on their own: idle ones once style no
        // longer applies them, and finished ones that have no effect.
        let keep = self.is_pending() ||
            self.pending_finish_notification.get() ||
            match self.play_state() {
                AnimationPlayState::Idle => self.style_target.get().is_some(),
                AnimationPlayState::Finished => self.is_in_effect(),
                _ => true,
            };
        if !keep {
            self.remove_script_style(cx);
            self.registered.set(false);
        }
        keep
    }

    /// <https://drafts.csswg.org/web-animations-1/#in-effect>
    fn is_in_effect(&self) -> bool {
        self.associated_effect.get().is_some_and(|effect| {
            effect
                .compute_timing(self.current_time(), self.playback_rate.get())
                .active_time
                .is_some()
        })
    }

    /// <https://drafts.csswg.org/web-animations-1/#relevant-animation>
    pub(crate) fn is_relevant(&self) -> bool {
        let Some(effect) = self.associated_effect.get() else {
            return false;
        };
        let playback_rate = self.playback_rate.get();
        let times = effect.compute_timing(self.current_time(), playback_rate);
        // > An animation effect is in effect if its active time is not unresolved.
        if times.active_time.is_some() {
            return true;
        }
        // > An animation effect is current if any of the following conditions are true: the
        // > animation effect is in play, or the animation effect is associated with an
        // > animation with a playback rate > 0 and the animation effect is in the before
        // > phase, or [..] with a playback rate < 0 and the animation effect is in the after
        // > phase.
        match times.phase {
            Some(EffectPhase::Active) => self.play_state() != AnimationPlayState::Finished,
            Some(EffectPhase::Before) => playback_rate > 0.0,
            Some(EffectPhase::After) => playback_rate < 0.0,
            None => false,
        }
    }

    /// Called when the timing or keyframes of the associated effect change.
    pub(crate) fn effect_timing_changed(&self, cx: &mut JSContext) {
        self.last_style_sample.set(None);
        self.update_finished_state(cx, false, false);
        self.update_style(cx);
    }

    /// Give style the current state of this animation, so that it applies the associated
    /// effect to its target.
    fn update_style(&self, cx: &mut JSContext) {
        // Make sure that an animation frame dispatches the events of the change.
        if self.css.borrow().is_some() {
            let global = self.global();
            let window = global.as_window();
            window
                .Document()
                .animation_manager()
                .note_animation_changed(window);
        }

        // A CSS animation or transition whose keyframes come from style is applied by style.
        if self.css.borrow().is_some() && self.update_css_style(cx) {
            self.remove_script_style(cx);
            self.register_with_document();
            return;
        }

        let effect = self.associated_effect.get();
        let keyframe_effect = effect
            .as_ref()
            .and_then(|effect| effect.downcast::<KeyframeEffect>());
        let target = keyframe_effect.and_then(|effect| effect.target_for_style());

        let global = self.global();
        let document = global.as_window().Document();
        let animations = document.animation_manager().sets();

        // Remove this animation from the element that it no longer targets.
        let old_target = self.style_target.get();
        if let Some(old_target) = old_target.as_ref() {
            if target.as_deref() != Some(&**old_target) {
                let node = old_target.upcast::<Node>();
                animations.remove_script_animation(
                    &AnimationSetKey::new_for_non_pseudo(node.to_opaque()),
                    self.unique_id,
                );
                node.dirty(cx.no_gc(), NodeDamage::Style);
                self.style_target.set(None);
                self.last_style_sample.set(None);
            }
        }

        let (Some(effect), Some(keyframe_effect), Some(target)) =
            (effect.as_ref(), keyframe_effect, target)
        else {
            return;
        };

        let play_state = self.play_state();
        if play_state == AnimationPlayState::Idle && !self.is_pending() {
            // An idle animation has no effect.
            if old_target.is_some() {
                let node = target.upcast::<Node>();
                animations.remove_script_animation(
                    &AnimationSetKey::new_for_non_pseudo(node.to_opaque()),
                    self.unique_id,
                );
                node.dirty(cx.no_gc(), NodeDamage::Style);
                self.style_target.set(None);
                self.last_style_sample.set(None);
            }
            return;
        }

        let times = effect.compute_timing(self.current_time(), self.playback_rate.get());
        let sample = StyleSample {
            iteration_progress: times.active_time.and(times.iteration_progress),
            before_flag: times.before_flag,
            needs_ticks: play_state == AnimationPlayState::Running || self.is_pending(),
        };
        if self.last_style_sample.get() == Some(sample) && old_target.is_some() {
            return;
        }
        self.last_style_sample.set(Some(sample));

        let node = target.upcast::<Node>();
        animations.update_script_animation(
            AnimationSetKey::new_for_non_pseudo(node.to_opaque()),
            ScriptAnimation::new(
                self.unique_id,
                keyframe_effect.keyframes_for_style(),
                keyframe_effect.composite_for_style(),
                sample.iteration_progress,
                sample.before_flag,
                sample.needs_ticks,
            ),
        );
        node.set_flag(NodeFlags::HAS_EVER_ANIMATED, true);
        node.dirty(cx.no_gc(), NodeDamage::Style);
        self.style_target.set(Some(&target));
        self.register_with_document();
    }
}

/// A computed keyframe of a CSS animation or transition, for `getKeyframes()`.
pub(crate) struct CssComputedKeyframe {
    pub(crate) offset: f64,
    pub(crate) easing: DOMString,
    pub(crate) composite: CompositeOperationOrAuto,
    /// The IDL attribute names of the properties, with their serialized values.
    pub(crate) values: Vec<(String, String)>,
}

/// The IDL attribute name and serialized value of an animated value.
fn serialize_animation_value(value: &AnimationValue) -> Option<(String, String)> {
    let name = animation_property_name_to_idl_attribute_name(&value.id().name());
    let mut serialization = String::new();
    value.uncompute().to_css(&mut serialization).ok()?;
    Some((name, serialization))
}

/// The position of a CSS animation or transition in the composite order, used to sort
/// its events together with the tree order of their targets.
pub(crate) struct CssEventOrder {
    is_transition: bool,
    pseudo_element_rank: u8,
    order: u64,
    name: String,
    unique_id: u64,
}

impl CssEventOrder {
    /// Compare the positions of two animations whose targets are the same element.
    pub(crate) fn cmp_same_target(&self, other: &CssEventOrder) -> CmpOrdering {
        other
            .is_transition
            .cmp(&self.is_transition)
            .then(self.pseudo_element_rank.cmp(&other.pseudo_element_rank))
            .then(self.order.cmp(&other.order))
            .then_with(|| {
                if self.is_transition {
                    self.name.cmp(&other.name)
                } else {
                    CmpOrdering::Equal
                }
            })
            .then(self.unique_id.cmp(&other.unique_id))
    }

    /// Whether this is the order of a CSS transition.
    pub(crate) fn is_transition(&self) -> bool {
        self.is_transition
    }
}

/// The rank of a pseudo-element in the composite order of animations.
fn pseudo_element_rank(pseudo_element: Option<PseudoElement>) -> u8 {
    match pseudo_element {
        None => 0,
        Some(PseudoElement::Marker) => 1,
        Some(PseudoElement::Before) => 2,
        Some(PseudoElement::After) => 4,
        Some(_) => 3,
    }
}

impl Animation {
    /// Create the `CSSAnimation` for an animation that style generated.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_css_animation(
        cx: &mut JSContext,
        window: &Window,
        owner: &Element,
        key: AnimationSetKey,
        style_id: u64,
        name: &str,
        name_index: usize,
        timing: CssEffectTiming,
        paused: bool,
    ) -> DomRoot<Animation> {
        let animation =
            DomRoot::upcast::<Animation>(CSSAnimation::new(cx, window, DOMString::from(name)));
        animation.initialize_css(
            cx,
            window,
            owner,
            CssState {
                is_transition: false,
                key,
                style_id,
                name: name.to_owned(),
                order: name_index as u64,
                overridden: 0,
                style_paused: paused,
                style_timing: timing,
                previous_phase: CssPhase::Idle,
                previous_iteration: None,
                previous_local_time: None,
            },
            None,
        );
        // > When an animation is created, if animation-play-state is paused, the animation
        // > is paused, otherwise it plays.
        if paused {
            let _ = animation.pause_an_animation(cx);
        } else {
            let _ = animation.play_an_animation(cx, true);
        }
        animation
    }

    /// Create the `CSSTransition` for a transition that style generated.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_css_transition(
        cx: &mut JSContext,
        window: &Window,
        owner: &Element,
        key: AnimationSetKey,
        style_id: u64,
        property: &str,
        generation: u64,
        timing: CssEffectTiming,
        easing: SpecifiedTimingFunction,
    ) -> DomRoot<Animation> {
        let animation =
            DomRoot::upcast::<Animation>(CSSTransition::new(cx, window, DOMString::from(property)));
        animation.initialize_css(
            cx,
            window,
            owner,
            CssState {
                is_transition: true,
                key,
                style_id,
                name: property.to_owned(),
                order: generation,
                overridden: 0,
                style_paused: false,
                style_timing: timing,
                previous_phase: CssPhase::Idle,
                previous_iteration: None,
                previous_local_time: None,
            },
            Some(easing),
        );
        let _ = animation.play_an_animation(cx, true);
        animation
    }

    fn initialize_css(
        &self,
        cx: &mut JSContext,
        window: &Window,
        owner: &Element,
        state: CssState,
        easing: Option<SpecifiedTimingFunction>,
    ) {
        let pseudo_selector = state
            .key
            .pseudo_element
            .map(|pseudo_element| DOMString::from(pseudo_element.to_css_string()));
        let timing = state.style_timing;
        *self.css.borrow_mut() = Some(state);
        self.owning_element.set(Some(owner));
        let document = window.Document();
        self.set_the_timeline(cx, Some(document.Timeline().upcast()));
        let effect = KeyframeEffect::new_for_css(cx, window, owner, pseudo_selector);
        effect
            .upcast::<AnimationEffect>()
            .set_css_timing(&timing, 0, easing);
        self.css_effect.set(Some(&effect));
        self.set_the_associated_effect(cx, Some(effect.upcast()));
    }

    /// Whether this is a CSS animation or transition that still has an owning element.
    fn has_owning_element(&self) -> bool {
        self.owning_element.get().is_some()
    }

    /// Update a CSS animation from the new values of the `animation-*` properties.
    pub(crate) fn update_from_css_style(
        &self,
        cx: &mut JSContext,
        timing: CssEffectTiming,
        paused: bool,
        name_index: usize,
    ) {
        let (timing_changed, overridden_flags, play_state_changed) = {
            let mut css = self.css.borrow_mut();
            let Some(css) = css.as_mut() else {
                return;
            };
            css.order = name_index as u64;
            let timing_changed = css.style_timing != timing;
            css.style_timing = timing;
            let play_state_changed = css.style_paused != paused;
            css.style_paused = paused;
            (timing_changed, css.overridden, play_state_changed)
        };

        // > After successfully setting the effect of a CSSAnimation to null or some
        // > AnimationEffect other than the original KeyframeEffect, changes to the
        // > animation-* properties other than animation-name and animation-play-state are not
        // > reflected in that animation.
        let css_effect = self.css_effect.get();
        if timing_changed &&
            css_effect.is_some() &&
            self.associated_effect.get().as_deref() ==
                css_effect.as_deref().map(Castable::upcast)
        {
            if let Some(effect) = css_effect {
                effect
                    .upcast::<AnimationEffect>()
                    .set_css_timing(&timing, overridden_flags, None);
                self.effect_timing_changed(cx);
            }
        }

        // > After a successful call to play() or pause() on a CSSAnimation, any subsequent
        // > change to the animation-play-state will no longer cause the CSSAnimation to be
        // > played or paused.
        if play_state_changed &&
            overridden_flags & overridden::PLAY_STATE == 0 &&
            self.play_state() != AnimationPlayState::Idle
        {
            if paused {
                let _ = self.pause_an_animation(cx);
            } else {
                let _ = self.play_an_animation(cx, false);
            }
        }
    }

    /// Cancel a CSS animation or transition that style no longer generates, and disassociate
    /// it from its owning element.
    pub(crate) fn cancel_from_style(&self, cx: &mut JSContext) {
        self.cancel_an_animation(cx);
        self.owning_element.set(None);
        self.update_style(cx);
    }

    /// Note that script changed the given [`overridden`] properties of a CSS animation, which
    /// style then no longer changes.
    pub(crate) fn override_css_properties(&self, flags: u8) {
        if let Some(css) = self.css.borrow_mut().as_mut() {
            if !css.is_transition {
                css.overridden |= flags;
            }
        }
    }

    /// The computed keyframes of a CSS animation or transition.
    ///
    /// <https://drafts.csswg.org/css-animations-2/#keyframes>
    pub(crate) fn css_computed_keyframes(
        &self,
        cx: &mut JSContext,
    ) -> Option<Vec<CssComputedKeyframe>> {
        self.flush_css_style(cx);
        let (key, style_id, is_transition) = {
            let css = self.css.borrow();
            let css = css.as_ref()?;
            (css.key.clone(), css.style_id, css.is_transition)
        };
        let global = self.global();
        let document = global.as_window().Document();
        let animations = document.animation_manager().sets();
        let sets = animations.sets.read();
        let set = sets.get(&key)?;
        if is_transition {
            let transition = set
                .transitions
                .iter()
                .find(|transition| transition.id == style_id)?;
            let property_animation = &transition.property_animation;
            let keyframe = |offset, value: &AnimationValue| CssComputedKeyframe {
                offset,
                easing: DOMString::from_static("linear"),
                composite: CompositeOperationOrAuto::Auto,
                values: serialize_animation_value(value).into_iter().collect(),
            };
            return Some(vec![
                keyframe(0.0, property_animation.from_value()),
                keyframe(1.0, property_animation.to_value()),
            ]);
        }
        let animation = set
            .animations
            .iter()
            .find(|animation| animation.id == style_id)?;
        Some(
            animation
                .css_keyframes()
                .into_iter()
                .map(|keyframe| CssComputedKeyframe {
                    offset: keyframe.offset,
                    easing: DOMString::from(style_traits::ToCss::to_css_string(
                        &keyframe.timing_function,
                    )),
                    composite: match keyframe.composite {
                        None => CompositeOperationOrAuto::Auto,
                        Some(AnimationComposition::Replace) => CompositeOperationOrAuto::Replace,
                        Some(AnimationComposition::Add) => CompositeOperationOrAuto::Add,
                        Some(AnimationComposition::Accumulate) => {
                            CompositeOperationOrAuto::Accumulate
                        },
                    },
                    values: keyframe
                        .values
                        .iter()
                        .filter_map(serialize_animation_value)
                        .collect(),
                })
                .collect(),
        )
    }

    /// Bring the state of a CSS animation or transition up to date with the style of the
    /// document, as script observes the result.
    fn flush_css_style(&self, cx: &mut JSContext) {
        if self.css.borrow().is_none() {
            return;
        }
        self.global()
            .as_window()
            .layout_reflow(cx, QueryMsg::StyleQuery);
    }

    /// The animation and transition sets key, style identifier and kind of the style
    /// animation that applies this CSS animation or transition, if style applies it: its
    /// effect is the one that style created, with the keyframes from style and the
    /// owning element as target.
    fn css_link_for_style(&self) -> Option<DomRoot<Element>> {
        let owner = self.owning_element.get()?;
        let effect = self.css_effect.get()?;
        if self.associated_effect.get().as_deref() != Some(effect.upcast()) ||
            !effect.uses_css_keyframes()
        {
            return None;
        }
        let key = self.css.borrow().as_ref()?.key.clone();
        let pseudo_selector = key
            .pseudo_element
            .map(|pseudo_element| DOMString::from(pseudo_element.to_css_string()));
        if effect
            .upcast::<AnimationEffect>()
            .associated_animation()
            .is_none() ||
            effect.target_pseudo_selector() != pseudo_selector
        {
            return None;
        }
        let target = crate::dom::bindings::codegen::Bindings::KeyframeEffectBinding::KeyframeEffectMethods::GetTarget(&*effect)?;
        (target == owner).then_some(owner)
    }

    /// Give style the timing of this CSS animation or transition. Returns whether style
    /// applies it, see [`Self::css_link_for_style`]; otherwise style is told that it has
    /// no effect, and it is applied like other animations.
    fn update_css_style(&self, cx: &mut JSContext) -> bool {
        let link = self.css_link_for_style();
        let (key, style_id, is_transition) = {
            let css = self.css.borrow();
            let Some(css) = css.as_ref() else {
                return false;
            };
            (css.key.clone(), css.style_id, css.is_transition)
        };

        let play_state = self.play_state();
        let pending = self.is_pending();
        let sample = match (&link, self.associated_effect.get()) {
            (Some(_), Some(effect)) if play_state != AnimationPlayState::Idle || pending => {
                let times = effect.compute_timing(self.current_time(), self.playback_rate.get());
                StyleSample {
                    iteration_progress: times.active_time.and(times.iteration_progress),
                    before_flag: times.before_flag,
                    needs_ticks: play_state == AnimationPlayState::Running || pending,
                }
            },
            _ => StyleSample::default(),
        };
        let finished = play_state == AnimationPlayState::Finished;
        let last_sample = self.last_style_sample.get();
        if last_sample == Some(sample) && (!is_transition || !finished) {
            return link.is_some();
        }
        self.last_style_sample.set(Some(sample));

        let state = if is_transition && finished {
            AnimationState::Finished
        } else if sample.needs_ticks {
            AnimationState::Running
        } else {
            AnimationState::Paused(0.0)
        };
        let timing = ScriptTiming {
            iteration_progress: sample.iteration_progress,
            before_flag: sample.before_flag,
        };
        let global = self.global();
        let document = global.as_window().Document();
        let animations = document.animation_manager().sets();
        {
            let mut sets = animations.sets.write();
            if let Some(set) = sets.get_mut(&key) {
                if is_transition {
                    if let Some(transition) = set
                        .transitions
                        .iter_mut()
                        .find(|transition| transition.id == style_id)
                    {
                        transition.script_timing = Some(timing);
                        if transition.state != AnimationState::Canceled {
                            transition.state = state;
                        }
                        set.dirty = true;
                    }
                } else if let Some(animation) = set
                    .animations
                    .iter_mut()
                    .find(|animation| animation.id == style_id)
                {
                    animation.script_timing = Some(timing);
                    if animation.state != AnimationState::Canceled {
                        animation.state = state;
                    }
                    set.dirty = true;
                }
            }
        }
        if let Some(owner) = self.owning_element.get() {
            let node = owner.upcast::<Node>();
            node.dirty(cx.no_gc(), NodeDamage::Style);
        }
        document.animation_manager().note_css_timing_changed();
        link.is_some()
    }

    /// Stop applying this animation as an animation created by script.
    fn remove_script_style(&self, cx: &mut JSContext) {
        let Some(old_target) = self.style_target.get() else {
            return;
        };
        let global = self.global();
        let document = global.as_window().Document();
        let node = old_target.upcast::<Node>();
        document.animation_manager().sets().remove_script_animation(
            &AnimationSetKey::new_for_non_pseudo(node.to_opaque()),
            self.unique_id,
        );
        node.dirty(cx.no_gc(), NodeDamage::Style);
        self.style_target.set(None);
        self.last_style_sample.set(None);
    }

    /// The phase of this CSS animation or transition for event dispatch, and its current
    /// iteration.
    fn css_phase(&self, is_transition: bool, previous_phase: CssPhase) -> (CssPhase, Option<f64>) {
        let current_time = self.current_time();
        let (phase, iteration) = match self.associated_effect.get() {
            None => (
                match current_time {
                    None => CssPhase::Idle,
                    Some(time) if time < 0.0 => CssPhase::Before,
                    Some(_) => CssPhase::After,
                },
                None,
            ),
            Some(effect) => {
                let times = effect.compute_timing(current_time, self.playback_rate.get());
                let phase = match times.phase {
                    None => CssPhase::Idle,
                    Some(EffectPhase::Before) => CssPhase::Before,
                    Some(EffectPhase::Active) => CssPhase::Active,
                    Some(EffectPhase::After) => CssPhase::After,
                };
                (phase, times.current_iteration)
            },
        };
        // > If the transition has a pending play task or a pending pause task and its phase
        // > was previously idle or pending, the transition phase is pending.
        if is_transition &&
            self.is_pending() &&
            matches!(previous_phase, CssPhase::Idle | CssPhase::Pending)
        {
            return (CssPhase::Pending, iteration);
        }
        (phase, iteration)
    }

    /// The interval start and interval end used for the elapsed time of events.
    fn css_interval(&self) -> (f64, f64) {
        let Some(effect) = self.associated_effect.get() else {
            return (0.0, 0.0);
        };
        let start_delay = effect.start_delay();
        let active_duration = effect.active_duration();
        // > interval start = max(min(-start delay, active duration), 0)
        // > interval end = max(min(associated effect end - start delay, active duration), 0)
        (
            (-start_delay).min(active_duration).max(0.0),
            (effect.end_time() - start_delay)
                .min(active_duration)
                .max(0.0),
        )
    }

    /// Queue the CSS animation or transition events for the change of phase since the
    /// last animation frame.
    ///
    /// <https://drafts.csswg.org/css-animations-2/#event-dispatch>
    /// <https://drafts.csswg.org/css-transitions-2/#event-dispatch>
    fn update_css_events(&self) {
        let Some(owner) = self.owning_element.get() else {
            return;
        };
        let (is_transition, previous_phase, previous_iteration) = {
            let css = self.css.borrow();
            let Some(css) = css.as_ref() else {
                return;
            };
            (
                css.is_transition,
                css.previous_phase,
                css.previous_iteration,
            )
        };
        // > Each time a new animation frame is established and the animation does not have a
        // > pending play task or pending pause task, the events to dispatch are determined.
        if !is_transition && self.is_pending() {
            return;
        }
        let (phase, iteration) = self.css_phase(is_transition, previous_phase);
        // > not idle and not after → idle: cancel, with the active time at the moment it
        // > was canceled. This happens when the timeline of the animation is removed.
        if phase == CssPhase::Idle && !matches!(previous_phase, CssPhase::Idle | CssPhase::After) {
            self.queue_css_cancel_event();
            return;
        }
        if let Some(css) = self.css.borrow_mut().as_mut() {
            css.previous_phase = phase;
            css.previous_iteration = iteration;
            css.previous_local_time = self.current_time();
        }

        use CssPhase::*;
        use TransitionOrAnimationEventType::*;
        let (interval_start, interval_end) = self.css_interval();
        let mut events: Vec<(TransitionOrAnimationEventType, f64)> = Vec::new();
        if is_transition {
            match (previous_phase, phase) {
                (Idle, Pending | Before) => events.push((TransitionRun, interval_start)),
                (Idle, Active) => {
                    events.push((TransitionRun, interval_start));
                    events.push((TransitionStart, interval_start));
                },
                (Idle, After) => {
                    events.push((TransitionRun, interval_start));
                    events.push((TransitionStart, interval_start));
                    events.push((TransitionEnd, interval_end));
                },
                (Pending | Before, Active) => events.push((TransitionStart, interval_start)),
                (Pending | Before, After) => {
                    events.push((TransitionStart, interval_start));
                    events.push((TransitionEnd, interval_end));
                },
                (Active, After) => events.push((TransitionEnd, interval_end)),
                (Active, Before) => events.push((TransitionEnd, interval_start)),
                (After, Active) => events.push((TransitionStart, interval_end)),
                (After, Before) => {
                    events.push((TransitionStart, interval_end));
                    events.push((TransitionEnd, interval_start));
                },
                _ => {},
            }
        } else {
            match (previous_phase, phase) {
                (Idle | Before, Active) => events.push((AnimationStart, interval_start)),
                (Idle | Before, After) => {
                    events.push((AnimationStart, interval_start));
                    events.push((AnimationEnd, interval_end));
                },
                (Active, Before) => events.push((AnimationEnd, interval_start)),
                (Active, Active) if previous_iteration != iteration => {
                    if let (Some(previous), Some(current), Some(effect)) =
                        (previous_iteration, iteration, self.associated_effect.get())
                    {
                        // > If the previous current iteration is greater than the current
                        // > iteration, let iteration boundary be current iteration + 1,
                        // > otherwise let it be current iteration.
                        let boundary = if previous > current {
                            current + 1.0
                        } else {
                            current
                        };
                        events.push((
                            AnimationIteration,
                            (boundary - effect.iteration_start()) * effect.iteration_duration(),
                        ));
                    }
                },
                (Active, After) => events.push((AnimationEnd, interval_end)),
                (After, Active) => events.push((AnimationStart, interval_end)),
                (After, Before) => {
                    events.push((AnimationStart, interval_end));
                    events.push((AnimationEnd, interval_start));
                },
                _ => {},
            }
        }
        // The events of one change of phase keep their order: they share the earliest of
        // their scheduled event times.
        let scheduled_event_time = events
            .iter()
            .filter_map(|(_, elapsed_time)| self.css_scheduled_event_time(*elapsed_time))
            .min_by(|a, b| a.total_cmp(b));
        for (event_type, elapsed_time) in events {
            self.queue_css_event_at(&owner, event_type, elapsed_time, scheduled_event_time);
        }
    }

    /// The time of the timeline at which an event with the given elapsed time is scheduled,
    /// used to sort events.
    ///
    /// <https://drafts.csswg.org/web-animations-1/#scheduled-event-time>
    fn css_scheduled_event_time(&self, elapsed_time: f64) -> Option<f64> {
        let start_time = self.start_time.get()?;
        let playback_rate = self.playback_rate.get();
        if playback_rate == 0.0 {
            return None;
        }
        let start_delay = self
            .associated_effect
            .get()
            .map_or(0.0, |effect| effect.start_delay());
        Some(start_time + (start_delay + elapsed_time) / playback_rate)
    }

    /// Queue the cancel event of a CSS animation or transition that is being canceled.
    fn queue_css_cancel_event(&self) {
        let Some(owner) = self.owning_element.get() else {
            return;
        };
        let (is_transition, previous_phase) = {
            let mut css = self.css.borrow_mut();
            let Some(css) = css.as_mut() else {
                return;
            };
            let previous_phase = std::mem::replace(&mut css.previous_phase, CssPhase::Idle);
            (css.is_transition, previous_phase)
        };
        if matches!(previous_phase, CssPhase::Idle | CssPhase::After) {
            return;
        }
        // > The active time of the animation at the moment it was canceled calculated using
        // > a fill mode of both.
        let local_time = self
            .current_time()
            .or_else(|| self.css.borrow().as_ref()?.previous_local_time)
            .unwrap_or(0.0);
        let elapsed_time = self
            .associated_effect
            .get()
            .map_or(0.0, |effect| effect.active_time_with_fill_both(local_time));
        let event_type = if is_transition {
            TransitionOrAnimationEventType::TransitionCancel
        } else {
            TransitionOrAnimationEventType::AnimationCancel
        };
        self.queue_css_event(&owner, event_type, elapsed_time);
    }

    fn queue_css_event(
        &self,
        owner: &Element,
        event_type: TransitionOrAnimationEventType,
        elapsed_time: f64,
    ) {
        // Cancel events have an unresolved scheduled event time, so they come before the
        // other events, like those of an animation that replaces the canceled one.
        self.queue_css_event_at(owner, event_type, elapsed_time, None);
    }

    fn queue_css_event_at(
        &self,
        owner: &Element,
        event_type: TransitionOrAnimationEventType,
        elapsed_time: f64,
        scheduled_event_time: Option<f64>,
    ) {
        let (pseudo_element, name, order) = {
            let css = self.css.borrow();
            let Some(css) = css.as_ref() else {
                return;
            };
            // The composite order of the animation when the event is queued, before a
            // canceled animation loses its owning element.
            let order = CssEventOrder {
                is_transition: css.is_transition,
                pseudo_element_rank: pseudo_element_rank(css.key.pseudo_element),
                order: css.order,
                name: css.name.clone(),
                unique_id: self.unique_id,
            };
            (css.key.pseudo_element, css.name.clone(), order)
        };
        let global = self.global();
        let window = global.as_window();
        let document = window.Document();
        let manager = document.animation_manager();
        manager.queue_css_event(
            self,
            owner,
            pseudo_element,
            event_type,
            name,
            elapsed_time / 1000.0,
            scheduled_event_time,
            order,
        );
        manager.handle_animation_presence_or_pending_events_change(window);
    }

    /// The target element and pseudo-element of the effect of this animation, for
    /// `getAnimations()`.
    pub(crate) fn effect_target(&self) -> Option<(DomRoot<Element>, bool)> {
        let effect = self.associated_effect.get()?;
        let effect = effect.downcast::<KeyframeEffect>()?;
        let target = crate::dom::bindings::codegen::Bindings::KeyframeEffectBinding::KeyframeEffectMethods::GetTarget(effect)?;
        Some((target, effect.target_pseudo_selector().is_some()))
    }

    /// Compare two animations in the composite order.
    ///
    /// <https://drafts.csswg.org/web-animations-1/#animation-composite-order>
    pub(crate) fn composite_order(&self, other: &Animation, no_gc: &NoGC) -> CmpOrdering {
        // CSS transitions with an owning element come first, then CSS animations with an
        // owning element, then other animations.
        let class = |animation: &Animation| match animation.css.borrow().as_ref() {
            Some(css) if animation.has_owning_element() => {
                if css.is_transition {
                    0
                } else {
                    1
                }
            },
            _ => 2,
        };
        let (class_a, class_b) = (class(self), class(other));
        if class_a != class_b {
            return class_a.cmp(&class_b);
        }
        if class_a == 2 {
            return self.unique_id.cmp(&other.unique_id);
        }

        // Sort by the tree order of the owning elements, and pseudo-elements after their
        // element.
        let (Some(owner_a), Some(owner_b)) =
            (self.owning_element.get(), other.owning_element.get())
        else {
            return self.unique_id.cmp(&other.unique_id);
        };
        let css_a = self.css.borrow();
        let css_b = other.css.borrow();
        let (Some(css_a), Some(css_b)) = (css_a.as_ref(), css_b.as_ref()) else {
            return self.unique_id.cmp(&other.unique_id);
        };
        if owner_a != owner_b {
            let (node_a, node_b) = (owner_a.upcast::<Node>(), owner_b.upcast::<Node>());
            return if node_a.is_before(no_gc, node_b) {
                CmpOrdering::Less
            } else {
                CmpOrdering::Greater
            };
        }
        let rank = pseudo_element_rank(css_a.key.pseudo_element)
            .cmp(&pseudo_element_rank(css_b.key.pseudo_element));
        if rank != CmpOrdering::Equal {
            return rank;
        }
        // Transitions sort by transition generation, then by property name; animations by
        // the position of their name in animation-name.
        css_a
            .order
            .cmp(&css_b.order)
            .then_with(|| {
                if css_a.is_transition {
                    css_a.name.cmp(&css_b.name)
                } else {
                    CmpOrdering::Equal
                }
            })
            .then_with(|| self.unique_id.cmp(&other.unique_id))
    }
}

impl AnimationMethods<crate::DomTypeHolder> for Animation {
    // <https://drafts.csswg.org/web-animations-1/#dom-animation-onfinish>
    event_handler!(finish, GetOnfinish, SetOnfinish);

    // <https://drafts.csswg.org/web-animations-1/#dom-animation-oncancel>
    event_handler!(cancel, GetOncancel, SetOncancel);

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-animation>
    fn Constructor(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        effect: Option<&AnimationEffect>,
        timeline: Option<Option<&AnimationTimeline>>,
    ) -> DomRoot<Self> {
        // Step 1. Let animation be a new Animation object.
        let animation = Animation::new_with_proto(cx, window.upcast(), proto);

        // Step 2. Run the procedure to set the timeline of an animation on animation passing timeline
        // as the new timeline; or, if the timeline argument is missing, passing the default document
        // timeline of the Document associated with the Window that is the current global object.
        match timeline {
            Some(timeline) => animation.set_the_timeline(cx, timeline),
            None => {
                let document = window.Document();
                let timeline = document.Timeline();
                animation.set_the_timeline(cx, Some(timeline.upcast()));
            },
        }

        // Step 3. Run the procedure to set the associated effect of an animation on animation passing
        // source as the new effect.
        animation.set_the_associated_effect(cx, effect);

        animation
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-id>
    fn Id(&self) -> DOMString {
        self.id.borrow().clone()
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-id>
    fn SetId(&self, id: DOMString) {
        *self.id.borrow_mut() = id;
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-effect>
    fn GetEffect(&self, cx: &mut JSContext) -> Option<DomRoot<AnimationEffect>> {
        // The timing of the effect of a CSS animation reflects pending style changes.
        self.flush_css_style(cx);
        self.associated_effect.get()
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-effect>
    fn SetEffect(&self, cx: &mut JSContext, effect: Option<&AnimationEffect>) {
        // > Setting this attribute updates the object’s associated effect using
        // > the procedure to set the associated effect of an animation.
        self.set_the_associated_effect(cx, effect);
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-animation-effect-timeline-timeline>
    fn GetTimeline(&self) -> Option<DomRoot<AnimationTimeline>> {
        self.timeline.get()
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-animation-effect-timeline-timeline>
    fn SetTimeline(&self, cx: &mut JSContext, timeline: Option<&AnimationTimeline>) {
        // > Setting this attribute updates the object’s timeline using the procedure
        // > to set the timeline of an animation.
        self.set_the_timeline(cx, timeline);
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-starttime>
    fn GetStartTime(&self) -> Option<Finite<f64>> {
        self.start_time.get().and_then(Finite::new)
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-starttime>
    fn SetStartTime(&self, cx: &mut JSContext, start_time: Option<Finite<f64>>) {
        let was_paused = self.play_state() == AnimationPlayState::Paused;
        self.set_the_start_time(cx, start_time.map(|time| *time));
        // > After successfully setting the startTime of a CSSAnimation, if the play state
        // > changes to or from paused, animation-play-state no longer plays or pauses it.
        if was_paused != (self.play_state() == AnimationPlayState::Paused) {
            self.override_css_properties(overridden::PLAY_STATE);
        }
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-currenttime>
    fn GetCurrentTime(&self) -> Option<Finite<f64>> {
        // Adding zero turns a negative zero into zero.
        self.current_time()
            .and_then(|current_time| Finite::new(current_time + 0.0))
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-currenttime>
    fn SetCurrentTime(&self, cx: &mut JSContext, current_time: Option<Finite<f64>>) -> ErrorResult {
        self.set_the_current_time(cx, current_time.map(|time| *time))
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-playbackrate>
    fn PlaybackRate(&self) -> Finite<f64> {
        Finite::wrap(self.playback_rate.get())
    }

    /// <https://drafts.csswg.org/web-animations-1/#set-the-playback-rate>
    fn SetPlaybackRate(&self, cx: &mut JSContext, playback_rate: Finite<f64>) {
        // Step 1. Clear any pending playback rate on animation.
        self.pending_playback_rate.set(None);
        // Step 2. Let previous time be the value of the current time of animation before
        // changing the playback rate.
        let previous_time = self.current_time();
        // Step 3. Let previous playback rate be the current effective playback rate of
        // animation.
        // Step 4. Set the playback rate to new playback rate.
        self.playback_rate.set(*playback_rate);
        // Step 5. Perform the steps corresponding to the first matching condition from the
        // following, if any: [..] If previous time is resolved, set the current time of
        // animation to previous time.
        if previous_time.is_some() {
            let _ = self.set_the_current_time(cx, previous_time);
        } else {
            self.update_style(cx);
        }
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-playstate>
    fn PlayState(&self, cx: &mut JSContext) -> AnimationPlayState {
        // Style changes, like that of animation-play-state, may change the play state.
        self.flush_css_style(cx);
        self.play_state()
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-pending>
    fn Pending(&self, cx: &mut JSContext) -> bool {
        self.flush_css_style(cx);
        self.is_pending()
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-ready>
    fn Ready(&self, cx: &mut JSContext) -> RootedPromise {
        self.flush_css_style(cx);
        self.ready_promise(cx)
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-finished>
    fn Finished(&self, cx: &mut JSContext) -> RootedPromise {
        self.finished_promise(cx)
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-cancel>
    fn Cancel(&self, cx: &mut JSContext) {
        self.cancel_an_animation(cx);
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-finish>
    fn Finish(&self, cx: &mut JSContext) -> ErrorResult {
        self.finish_an_animation(cx)
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-play>
    fn Play(&self, cx: &mut JSContext) -> ErrorResult {
        self.flush_css_style(cx);
        self.play_an_animation(cx, true)?;
        // > After a successful call to play() or pause() on a CSSAnimation, any subsequent
        // > change to the animation-play-state will no longer cause the CSSAnimation to be
        // > played or paused.
        self.override_css_properties(overridden::PLAY_STATE);
        Ok(())
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-pause>
    fn Pause(&self, cx: &mut JSContext) -> ErrorResult {
        self.flush_css_style(cx);
        self.pause_an_animation(cx)?;
        self.override_css_properties(overridden::PLAY_STATE);
        Ok(())
    }

    /// <https://drafts.csswg.org/web-animations-1/#seamlessly-update-the-playback-rate>
    fn UpdatePlaybackRate(&self, cx: &mut JSContext, playback_rate: Finite<f64>) {
        // Step 1. Let previous play state be animation’s play state.
        let previous_play_state = self.play_state();
        // Step 2. Let animation’s pending playback rate be new playback rate.
        self.pending_playback_rate.set(Some(*playback_rate));
        // Step 3. Perform the steps corresponding to the first matching condition below:
        if self.is_pending() {
            // > If animation has a pending play task or a pending pause task, Abort these steps.
            return;
        }
        match previous_play_state {
            // > If previous play state is idle or paused, or animation’s current time is
            // > unresolved, Apply any pending playback rate on animation.
            AnimationPlayState::Idle | AnimationPlayState::Paused => {
                self.apply_any_pending_playback_rate();
                self.update_style(cx);
            },
            _ if self.current_time().is_none() => {
                self.apply_any_pending_playback_rate();
                self.update_style(cx);
            },
            AnimationPlayState::Finished => {
                // > If previous play state is finished,
                // > Let the unconstrained current time be the result of calculating the current
                // > time of animation substituting an unresolved time value for the hold time.
                let unconstrained_current_time = self.current_time_with_hold_time(None);
                // > Let animation’s start time be the result of evaluating the following
                // > expression: timeline time - (unconstrained current time / pending playback
                // > rate). If pending playback rate is zero, let animation’s start time be
                // > timeline time.
                if let (Some(timeline_time), Some(unconstrained_current_time)) =
                    (self.timeline_time(), unconstrained_current_time)
                {
                    let rate = *playback_rate;
                    self.start_time.set(Some(if rate == 0.0 {
                        timeline_time
                    } else {
                        timeline_time - unconstrained_current_time / rate
                    }));
                }
                // > Apply any pending playback rate on animation.
                self.apply_any_pending_playback_rate();
                // > Run the procedure to update an animation’s finished state for animation
                // > with the did seek flag set to false, and the synchronously notify flag set
                // > to false.
                self.update_finished_state(cx, false, false);
                self.update_style(cx);
            },
            AnimationPlayState::Running => {
                // > Otherwise, Run the procedure to play an animation for animation with the
                // > auto-rewind flag set to false.
                let _ = self.play_an_animation(cx, false);
            },
        }
    }

    /// <https://drafts.csswg.org/web-animations-1/#reverse-an-animation>
    fn Reverse(&self, cx: &mut JSContext) -> ErrorResult {
        // Step 1. If there is no timeline associated with animation, or the associated timeline
        // is inactive throw an "InvalidStateError" DOMException and abort these steps.
        if self.timeline_time().is_none() {
            return Err(Error::InvalidState(Some(
                "Cannot reverse an animation without an active timeline".to_owned(),
            )));
        }
        // Step 2. Let original pending playback rate be animation’s pending playback rate.
        let original_pending_playback_rate = self.pending_playback_rate.get();
        // Step 3. Let animation’s pending playback rate be the additive inverse of its
        // effective playback rate (i.e. -effective playback rate).
        self.pending_playback_rate
            .set(Some(-self.effective_playback_rate()));
        // Step 4. Run the steps to play an animation for animation with the auto-rewind flag
        // set to true. If the steps to play an animation throw an exception, set animation’s
        // pending playback rate to original pending playback rate and propagate the exception.
        let was_paused = self.play_state() == AnimationPlayState::Paused;
        let result = self.play_an_animation(cx, true);
        if result.is_err() {
            self.pending_playback_rate
                .set(original_pending_playback_rate);
        } else if was_paused != (self.play_state() == AnimationPlayState::Paused) {
            self.override_css_properties(overridden::PLAY_STATE);
        }
        result
    }
}
