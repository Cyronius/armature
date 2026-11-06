/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::cell::DomRefCell;
use script_bindings::codegen::GenericBindings::AnimationBinding::AnimationPlayState;
use script_bindings::codegen::GenericBindings::DocumentBinding::DocumentMethods;
use script_bindings::codegen::GenericBindings::WindowBinding::WindowMethods;
use script_bindings::error::{Error, ErrorResult};
use script_bindings::inheritance::Castable;
use script_bindings::num::Finite;
use script_bindings::reflector::reflect_dom_object_with_proto;
use script_bindings::root::DomRoot;
use script_bindings::str::DOMString;
use style::animation::{AnimationSetKey, ScriptAnimation};
use stylo_atoms::Atom;

use crate::dom::animationeffect::AnimationEffect;
use crate::dom::bindings::codegen::Bindings::AnimationBinding::AnimationMethods;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::MutNullableDom;
use crate::dom::element::Element;
use crate::dom::eventtarget::EventTarget;
use crate::dom::globalscope::GlobalScope;
use crate::dom::keyframeeffect::KeyframeEffect;
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
        }
    }

    fn new_with_proto(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
    ) -> DomRoot<Self> {
        let animation =
            reflect_dom_object_with_proto(cx, Box::new(Self::new_inherited()), global, proto);

        // > The current ready promise is initially a resolved Promise created using the
        // > procedure to create a new resolved Promise with the animation itself as its value.
        let ready_promise = Promise::new_resolved(cx, global, DomRoot::from_ref(&*animation));
        *animation.ready_promise.borrow_mut() = Some(ready_promise.to_traced());

        // > The current finished promise is initially a pending Promise object.
        let finished_promise = Promise::new(cx, global);
        *animation.finished_promise.borrow_mut() = Some(finished_promise.to_traced());
        animation
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
                    return Err(Error::InvalidState(None));
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
                    return Err(Error::InvalidState(None));
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
        global
            .as_window()
            .Document()
            .animation_manager()
            .queue_playback_event(self, event_type, current_time, self.timeline_time());
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
        ready_promise.reject_error(cx, Error::Abort(None));
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
            // Step 1.1. Run the procedure to reset an animation’s pending tasks on animation.
            self.reset_pending_tasks(cx);
            // Step 1.2. Reject the current finished promise with a DOMException named
            // "AbortError".
            // Step 1.3. Set the [[PromiseIsHandled]] internal slot of the current finished
            // promise to true.
            let finished_promise = self.finished_promise(cx);
            if finished_promise.is_pending() {
                finished_promise.reject_error(cx, Error::Abort(None));
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
            return Err(Error::InvalidState(None));
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

        let keep =
            self.play_state() != AnimationPlayState::Idle || self.style_target.get().is_some();
        if !keep {
            self.registered.set(false);
        }
        keep
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
        let effect = self.associated_effect.get();
        let keyframe_effect = effect
            .as_ref()
            .and_then(|effect| effect.downcast::<KeyframeEffect>());
        let target = keyframe_effect.and_then(|effect| effect.target());

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
    fn GetEffect(&self) -> Option<DomRoot<AnimationEffect>> {
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
        self.set_the_start_time(cx, start_time.map(|time| *time));
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
    fn PlayState(&self) -> AnimationPlayState {
        self.play_state()
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-pending>
    fn Pending(&self) -> bool {
        self.is_pending()
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-ready>
    fn Ready(&self, cx: &mut JSContext) -> RootedPromise {
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
        self.play_an_animation(cx, true)
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-pause>
    fn Pause(&self, cx: &mut JSContext) -> ErrorResult {
        self.pause_an_animation(cx)
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
            return Err(Error::InvalidState(None));
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
        let result = self.play_an_animation(cx, true);
        if result.is_err() {
            self.pending_playback_rate
                .set(original_pending_playback_rate);
        }
        result
    }
}
