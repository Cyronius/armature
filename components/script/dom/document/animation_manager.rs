/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The set of animations for a document.

#![cfg_attr(crown, allow(crown::jscontext_first_arg))]

use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;

use cssparser::ToCss;
use embedder_traits::{AnimationState as AnimationsPresentState, UntrustedNodeAddress};
use html5ever::ns;
use js::context::{JSContext, NoGC};
use layout_api::AnimatingImages;
use libc::c_void;
use paint_api::ImageUpdate;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use script_bindings::cell::DomRefCell;
use script_bindings::codegen::GenericBindings::AnimationBinding::AnimationReplaceState;
use script_bindings::codegen::GenericBindings::AnimationEffectBinding::{
    FillMode, PlaybackDirection,
};
use script_bindings::codegen::GenericBindings::WindowBinding::WindowMethods;
use script_bindings::refcounted::Trusted;
use serde::{Deserialize, Serialize};
use servo_base::id::PipelineId;
use servo_constellation_traits::ScriptToConstellationMessage;
use style::animation::{
    Animation, AnimationSetKey, AnimationState, DocumentAnimationSet, ElementAnimationSet,
    KeyframesIterationState, Transition,
};
use style::dom::OpaqueNode;
use style::properties::PropertyDeclarationIdSet;
use style::properties::longhands::animation_direction::computed_value::single_value::T as AnimationDirection;
use style::properties::longhands::animation_fill_mode::computed_value::single_value::T as AnimationFillMode;
use style::selector_parser::PseudoElement;
use style::values::computed::ToComputedValue;
use style::values::specified::TimingFunction as SpecifiedTimingFunction;
use stylo_atoms::Atom;
use timers::TimerId;

use crate::dom::NodeFlags;
use crate::dom::animation::CssEventOrder;
use crate::dom::animationeffect::CssEffectTiming;
use crate::dom::animationevent::AnimationEvent;
use crate::dom::animationplaybackevent::AnimationPlaybackEvent;
use crate::dom::bindings::codegen::Bindings::AnimationEventBinding::AnimationEventInit;
use crate::dom::bindings::codegen::Bindings::AnimationPlaybackEventBinding::AnimationPlaybackEventInit;
use crate::dom::bindings::codegen::Bindings::EventBinding::EventInit;
use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::codegen::Bindings::TransitionEventBinding::TransitionEventInit;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::bindings::str::DOMString;
use crate::dom::bindings::trace::NoTrace;
use crate::dom::element::Element;
use crate::dom::event::Event;
use crate::dom::node::{Node, NodeDamage, NodeTraits, from_untrusted_node_address};
use crate::dom::transitionevent::TransitionEvent;
use crate::dom::types::Animation as DomAnimation;
use crate::dom::window::Window;
use crate::event_loop::script_thread::with_script_thread;

/// The set of animations for a document.
#[derive(Default, JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct AnimationManager {
    /// The map of nodes to their animation states.
    #[no_trace]
    sets: DocumentAnimationSet,

    /// The set of [`AnimatingImages`] which is used to communicate the addition
    /// and removal of animating images from layout.
    #[no_trace]
    #[conditional_malloc_size_of]
    animating_images: Arc<RwLock<AnimatingImages>>,

    /// Whether or not we have animations that are running.
    has_running_animations: Cell<bool>,

    /// Whether the constellation was last told that this document has animations.
    animations_present: Cell<bool>,

    /// Whether script changed an animation since the last animation frame, so that the
    /// next animation frame dispatches its events.
    has_changed_animations: Cell<bool>,

    /// Whether the animations created by script are being updated for an animation frame.
    updating_animations: Cell<bool>,

    /// Whether a CSS animation or transition gave style new timing, so that the style of
    /// its element is out of date.
    css_timing_changed: Cell<bool>,

    /// A map of nodes with in-progress CSS transitions or pending events.
    rooted_animation_nodes: DomRefCell<FxHashMap<NoTrace<OpaqueNode>, Dom<Node>>>,

    /// A map of nodes with in-progress image animations. This is kept outside
    /// of [`Self::animating_images`] as that data structure is shared with layout.
    rooted_image_nodes: DomRefCell<FxHashMap<NoTrace<OpaqueNode>, Dom<Node>>>,

    /// A list of pending animation-related events.
    pending_events: DomRefCell<Vec<TransitionOrAnimationEvent>>,

    /// The timeline value at the last time all animations were marked dirty.
    /// This is used to prevent marking animations dirty when the timeline
    /// has not changed.
    timeline_value_at_last_dirty: Cell<f64>,

    /// The [`TimerId`] of the currently scheduled animated image update callback.
    #[no_trace]
    image_callback_timer_id: Cell<Option<TimerId>>,

    /// The animations created by script that have pending tasks or that may affect style,
    /// which are updated at every animation frame.
    script_animations: DomRefCell<Vec<Dom<DomAnimation>>>,

    /// The `finish` and `cancel` events of animations created by script, waiting to be
    /// dispatched.
    pending_playback_events: DomRefCell<Vec<PendingPlaybackEvent>>,

    /// The `CSSAnimation` and `CSSTransition` objects of the animations and transitions that
    /// style generated, by their identifier in the animation sets.
    css_animations: DomRefCell<FxHashMap<u64, Dom<DomAnimation>>>,

    /// The events of CSS animations and transitions, waiting to be dispatched.
    css_events: DomRefCell<Vec<CssEvent>>,

    /// <https://drafts.csswg.org/css-transitions-2/#current-transition-generation>
    transition_generation: Cell<u64>,
}

/// A queued event of a CSS animation or transition.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct CssEvent {
    animation: Dom<DomAnimation>,
    target: Dom<Element>,
    #[no_trace]
    pseudo_element: Option<PseudoElement>,
    event_type: TransitionOrAnimationEventType,
    name: String,
    /// The elapsed time, in seconds.
    elapsed_time: f64,
    /// <https://drafts.csswg.org/web-animations-1/#scheduled-event-time>
    scheduled_event_time: Option<f64>,
    #[no_trace]
    #[ignore_malloc_size_of = "Plain data"]
    order: CssEventOrder,
}

/// What to do with a CSS animation or transition after a restyle.
enum CssAnimationUpdate {
    CreateAnimation {
        key: AnimationSetKey,
        id: u64,
        name: String,
        name_index: usize,
        timing: CssEffectTiming,
        paused: bool,
    },
    CreateTransition {
        key: AnimationSetKey,
        id: u64,
        property: String,
        timing: CssEffectTiming,
        easing: SpecifiedTimingFunction,
    },
    Update {
        animation: DomRoot<DomAnimation>,
        timing: CssEffectTiming,
        paused: bool,
        name_index: usize,
    },
    Cancel(DomRoot<DomAnimation>),
}

/// The timing properties of the effect of a CSS animation.
fn css_animation_timing(animation: &Animation) -> CssEffectTiming {
    CssEffectTiming {
        duration: animation.duration * 1000.0,
        delay: animation.delay * 1000.0,
        iterations: match animation.iteration_state {
            KeyframesIterationState::Finite(_, max) => max,
            KeyframesIterationState::Infinite(_) => f64::INFINITY,
        },
        direction: match animation.direction {
            AnimationDirection::Normal => PlaybackDirection::Normal,
            AnimationDirection::Reverse => PlaybackDirection::Reverse,
            AnimationDirection::Alternate => PlaybackDirection::Alternate,
            AnimationDirection::AlternateReverse => PlaybackDirection::Alternate_reverse,
        },
        fill: match animation.fill_mode {
            AnimationFillMode::None => FillMode::None,
            AnimationFillMode::Forwards => FillMode::Forwards,
            AnimationFillMode::Backwards => FillMode::Backwards,
            AnimationFillMode::Both => FillMode::Both,
        },
    }
}

/// The timing properties of the effect of a CSS transition.
///
/// <https://drafts.csswg.org/css-transitions-2/#csstransition>
fn css_transition_timing(transition: &Transition) -> (CssEffectTiming, SpecifiedTimingFunction) {
    let timing = CssEffectTiming {
        duration: transition.property_animation.duration * 1000.0,
        delay: transition.effective_delay() * 1000.0,
        iterations: 1.0,
        direction: PlaybackDirection::Normal,
        fill: FillMode::Backwards,
    };
    let easing = SpecifiedTimingFunction::from_computed_value(
        transition.property_animation.timing_function(),
    );
    (timing, easing)
}

/// A queued `AnimationPlaybackEvent`.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct PendingPlaybackEvent {
    animation: Dom<DomAnimation>,
    #[no_trace]
    event_type: Atom,
    current_time: Option<f64>,
    timeline_time: Option<f64>,
    scheduled_event_time: Option<f64>,
    target: Option<Dom<Element>>,
    #[no_trace]
    #[ignore_malloc_size_of = "Plain data"]
    order: Option<CssEventOrder>,
}

struct PlaybackEventToDispatch {
    animation: DomRoot<DomAnimation>,
    event_type: Atom,
    current_time: Option<f64>,
    timeline_time: Option<f64>,
    scheduled_event_time: Option<f64>,
    target: Option<DomRoot<Element>>,
    order: Option<CssEventOrder>,
}

type CssEventToDispatch = (
    DomRoot<DomAnimation>,
    DomRoot<Element>,
    Option<PseudoElement>,
    TransitionOrAnimationEventType,
    String,
    f64,
    Option<f64>,
    CssEventOrder,
);

impl AnimationManager {
    pub(crate) fn new() -> Self {
        AnimationManager {
            sets: Default::default(),
            animating_images: Default::default(),
            has_running_animations: Cell::new(false),
            animations_present: Cell::new(false),
            has_changed_animations: Cell::new(false),
            updating_animations: Cell::new(false),
            css_timing_changed: Cell::new(false),
            rooted_animation_nodes: Default::default(),
            rooted_image_nodes: Default::default(),
            pending_events: Default::default(),
            timeline_value_at_last_dirty: Cell::new(0.0),
            image_callback_timer_id: Default::default(),
            script_animations: Default::default(),
            pending_playback_events: Default::default(),
            css_animations: Default::default(),
            css_events: Default::default(),
            transition_generation: Default::default(),
        }
    }

    /// Queue an event of a CSS animation or transition.
    pub(crate) fn queue_css_event(
        &self,
        animation: &DomAnimation,
        target: &Element,
        pseudo_element: Option<PseudoElement>,
        event_type: TransitionOrAnimationEventType,
        name: String,
        elapsed_time: f64,
        scheduled_event_time: Option<f64>,
        order: CssEventOrder,
    ) {
        self.css_events.borrow_mut().push(CssEvent {
            order,
            animation: Dom::from_ref(animation),
            target: Dom::from_ref(target),
            pseudo_element,
            event_type,
            name,
            elapsed_time,
            scheduled_event_time,
        });
    }

    /// The relevant animations whose effect target matches `filter`, in the composite
    /// order, for `getAnimations()`.
    pub(crate) fn relevant_animations(
        &self,
        no_gc: &NoGC,
        filter: impl Fn(&Element, Option<&DOMString>) -> bool,
    ) -> Vec<DomRoot<DomAnimation>> {
        let mut animations: Vec<DomRoot<DomAnimation>> = self
            .script_animations
            .borrow()
            .iter()
            .map(|animation| animation.as_rooted())
            .filter(|animation| {
                animation.is_relevant() &&
                    animation
                        .effect_target()
                        .is_some_and(|(target, pseudo)| filter(&target, pseudo.as_ref()))
            })
            .collect();
        animations.sort_by(|a, b| a.composite_order(b, no_gc));
        animations
    }

    /// Queue an `AnimationPlaybackEvent` for an animation created by script.
    pub(crate) fn queue_playback_event(
        &self,
        animation: &DomAnimation,
        event_type: Atom,
        current_time: Option<f64>,
        timeline_time: Option<f64>,
        scheduled_event_time: Option<f64>,
        css_order: Option<(DomRoot<Element>, CssEventOrder)>,
    ) {
        let (target, order) = match css_order {
            Some((target, order)) => (Some(Dom::from_ref(&*target)), Some(order)),
            None => (None, None),
        };
        self.pending_playback_events
            .borrow_mut()
            .push(PendingPlaybackEvent {
                animation: Dom::from_ref(animation),
                event_type,
                current_time,
                timeline_time,
                scheduled_event_time,
                target,
                order,
            });
    }

    /// Dispatch the queued `AnimationPlaybackEvent`s, as part of
    /// <https://drafts.csswg.org/web-animations-1/#update-animations-and-send-events>.
    fn send_playback_events(
        &self,
        window: &Window,
        cx: &mut js::context::JSContext,
        mut events: Vec<PlaybackEventToDispatch>,
    ) {
        let no_gc = cx.no_gc();
        events.sort_by(|a, b| {
            let time_order = match (a.scheduled_event_time, b.scheduled_event_time) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (Some(a), Some(b)) => a.total_cmp(&b),
            };
            time_order.then_with(|| match (&a.order, &b.order) {
                (Some(a_order), Some(b_order)) => b_order
                    .is_transition()
                    .cmp(&a_order.is_transition())
                    .then_with(|| {
                        let (Some(a_target), Some(b_target)) = (&a.target, &b.target) else {
                            return std::cmp::Ordering::Equal;
                        };
                        if a_target == b_target {
                            a_order.cmp_same_target(b_order)
                        } else if a_target
                            .upcast::<Node>()
                            .is_before(no_gc, b_target.upcast())
                        {
                            std::cmp::Ordering::Less
                        } else {
                            std::cmp::Ordering::Greater
                        }
                    }),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.animation.composite_order(&b.animation, no_gc),
            })
        });
        for event in events {
            let init = AnimationPlaybackEventInit {
                parent: EventInit {
                    bubbles: false,
                    cancelable: false,
                    composed: false,
                },
                currentTime: event.current_time.and_then(Finite::new),
                timelineTime: event.timeline_time.and_then(Finite::new),
            };
            AnimationPlaybackEvent::new(cx, window, event.event_type, &init)
                .upcast::<Event>()
                .fire(cx, event.animation.upcast());
        }
        self.handle_animation_presence_or_pending_events_change(window);
    }

    /// The animations that are updated at every animation frame, which include every
    /// animation that applies an effect to style.
    pub(crate) fn registered_animations(&self) -> Vec<DomRoot<DomAnimation>> {
        self.script_animations
            .borrow()
            .iter()
            .map(|animation| animation.as_rooted())
            .collect()
    }

    /// Add an animation created by script to the animations updated at every animation frame.
    pub(crate) fn register_script_animation(&self, animation: &DomAnimation) {
        self.script_animations
            .borrow_mut()
            .push(Dom::from_ref(animation));
    }

    /// Run the pending tasks of the animations created by script and update the style of
    /// their targets, as part of <https://drafts.csswg.org/web-animations-1/#update-animations-and-send-events>.
    pub(crate) fn update_script_animations(&self, cx: &mut JSContext) {
        self.has_changed_animations.set(false);
        self.updating_animations.set(true);
        self.update_script_animations_inner(cx);
        self.remove_replaced_animations(cx);
        self.updating_animations.set(false);
    }

    /// <https://drafts.csswg.org/web-animations-1/#remove-replaced-animations>
    fn remove_replaced_animations(&self, cx: &mut JSContext) {
        // > For every animation, animation, that has an associated animation effect whose
        // > effect target is a descendant of doc, and is replaceable, and has a replace
        // > state of active, and for which there exists for each target property of every
        // > animation effect associated with animation, an animation effect associated with
        // > a replaceable animation with a higher composite order than animation that
        // > includes the same target property, perform the following steps.
        let mut replaceable: Vec<(
            DomRoot<DomAnimation>,
            DomRoot<Element>,
            Option<PseudoElement>,
        )> = self
            .script_animations
            .borrow()
            .iter()
            .filter_map(|animation| {
                let animation = animation.as_rooted();
                let (target, pseudo) = animation.replaceable_target()?;
                Some((animation, target, pseudo))
            })
            .collect();
        if replaceable.len() < 2 {
            return;
        }
        replaceable.sort_by(|(a, _, _), (b, _, _)| a.composite_order(b, cx.no_gc()));

        let mut by_target: Vec<(
            DomRoot<Element>,
            Option<PseudoElement>,
            Vec<DomRoot<DomAnimation>>,
        )> = Vec::new();
        for (animation, target, pseudo) in replaceable {
            match by_target
                .iter_mut()
                .find(|(other_target, other_pseudo, _)| {
                    *other_target == target && *other_pseudo == pseudo
                }) {
                Some((_, _, animations)) => animations.push(animation),
                None => by_target.push((target, pseudo, vec![animation])),
            }
        }

        let mut removed = Vec::new();
        for (target, _, animations) in by_target {
            if animations.len() < 2 {
                continue;
            }
            // Logical properties are replaced by the physical properties that they map to
            // in the writing mode of the target.
            let Some(style) = target.style() else {
                continue;
            };
            let writing_mode = style.writing_mode;
            let mut covered = PropertyDeclarationIdSet::default();
            for animation in animations.iter().rev() {
                let Some(properties) = animation.target_properties(writing_mode) else {
                    continue;
                };
                if animation.replace_state() == AnimationReplaceState::Active &&
                    covered.contains_all(&properties)
                {
                    removed.push(animation.clone());
                } else {
                    for property in properties.iter() {
                        covered.insert(property);
                    }
                }
            }
        }

        // Queue all the remove events before any of them is dispatched, in composite order.
        removed.sort_by(|a, b| a.composite_order(b, cx.no_gc()));
        for animation in removed {
            animation.remove_replaced(cx);
        }
    }

    /// Note that script changed an animation outside of an animation frame, so that an
    /// animation frame happens to dispatch the events of the change.
    pub(crate) fn note_animation_changed(&self, window: &Window) {
        if self.updating_animations.get() || self.has_changed_animations.get() {
            return;
        }
        self.has_changed_animations.set(true);
        if self.animations_present.get() {
            // Animation ticks follow paints, so a document whose animations do not change its
            // rendering gets no further tick. Changing the state makes paint send one.
            for state in [
                AnimationsPresentState::NoAnimationsPresent,
                AnimationsPresentState::AnimationsPresent,
            ] {
                window.send_to_constellation(
                    ScriptToConstellationMessage::ChangeRunningAnimationsState(state),
                );
            }
            return;
        }
        self.handle_animation_presence_or_pending_events_change(window);
    }

    fn update_script_animations_inner(&self, cx: &mut JSContext) {
        let animations: Vec<DomRoot<DomAnimation>> = self
            .script_animations
            .borrow_mut()
            .drain(..)
            .map(|animation| animation.as_rooted())
            .collect();
        if animations.is_empty() {
            return;
        }
        let kept: Vec<DomRoot<DomAnimation>> = animations
            .into_iter()
            .filter(|animation| animation.update_for_animation_frame(cx))
            .collect();
        // Animations registered while updating come after the ones that were already registered.
        self.script_animations.borrow_mut().splice(
            0..0,
            kept.iter().map(|animation| Dom::from_ref(&**animation)),
        );
    }

    pub(crate) fn animating_images(&self) -> Arc<RwLock<AnimatingImages>> {
        self.animating_images.clone()
    }

    pub(crate) fn sets(&self) -> DocumentAnimationSet {
        self.sets.clone()
    }

    pub(crate) fn clear(&self) {
        self.sets.sets.write().clear();
        self.animating_images.write().node_to_state_map.clear();
        self.rooted_animation_nodes.borrow_mut().clear();
        self.rooted_image_nodes.borrow_mut().clear();
        self.pending_events.borrow_mut().clear();
        self.css_animations.borrow_mut().clear();
        self.css_events.borrow_mut().clear();
    }

    // Mark all animations dirty, if they haven't been marked dirty since the
    // specified `current_timeline_value`. Returns true if animations were marked
    // dirty or false otherwise.
    pub(crate) fn mark_animating_nodes_as_dirty(
        &self,
        no_gc: &NoGC,
        current_timeline_value: f64,
    ) -> bool {
        if current_timeline_value <= self.timeline_value_at_last_dirty.get() {
            return false;
        }
        self.timeline_value_at_last_dirty
            .set(current_timeline_value);

        let sets = self.sets.sets.read();
        let rooted_nodes = self.rooted_animation_nodes.borrow();
        // Animations and transitions whose timing script owns mark their nodes dirty when
        // their progress changes.
        for node in sets
            .iter()
            .filter(|(_, set)| {
                set.animations.iter().any(|animation| {
                    animation.script_timing.is_none() && animation.state != AnimationState::Canceled
                }) || set.transitions.iter().any(|transition| {
                    transition.script_timing.is_none() &&
                        transition.state != AnimationState::Canceled
                })
            })
            .filter_map(|(key, _)| rooted_nodes.get(&NoTrace(key.node)))
        {
            node.dirty(no_gc, NodeDamage::Style);
        }

        true
    }

    pub(crate) fn update_for_new_timeline_value(&self, window: &Window, now: f64) {
        let pipeline_id = window.pipeline_id();
        let mut sets = self.sets.sets.write();

        for (key, set) in sets.iter_mut() {
            self.start_pending_animations(key, set, now, pipeline_id);

            // When necessary, iterate our running animations to the next iteration.
            if now > self.timeline_value_at_last_dirty.get() {
                for animation in set.animations.iter_mut() {
                    if animation.script_timing.is_some() {
                        continue;
                    }
                    if animation.iterate_if_necessary(now) {
                        self.add_animation_event(
                            key,
                            animation,
                            TransitionOrAnimationEventType::AnimationIteration,
                            now,
                            pipeline_id,
                        );
                    }
                }
            }

            self.finish_running_animations(key, set, now, pipeline_id);
        }

        self.unroot_unused_nodes(&sets);
    }

    /// Cancel animations for the given node, if any exist.
    pub(crate) fn cancel_animations_for_node(&self, node: &Node) {
        if !node.get_flag(NodeFlags::HAS_EVER_ANIMATED) {
            return;
        }

        let opaque_node = node.to_opaque();
        let animation_keys = [
            AnimationSetKey::new_for_non_pseudo(opaque_node),
            AnimationSetKey::new_for_pseudo(opaque_node, PseudoElement::Before),
            AnimationSetKey::new_for_pseudo(opaque_node, PseudoElement::After),
        ];

        let mut animations = self.sets.sets.write();
        for key in animation_keys {
            if let Some(set) = animations.get_mut(&key) {
                set.cancel_all_animations();
            }
        }

        self.animating_images().write().remove(opaque_node);
        self.rooted_image_nodes
            .borrow_mut()
            .remove(&NoTrace(opaque_node));
    }

    /// This does five things:
    ///  - Cancel animations for any nodes that are no longer being rendered or delegating rendering.
    ///  - Process any new animations that were discovered after reflow.
    ///  - Collect pending events for any animations that changed state.
    ///  - Root any nodes with newly animating images
    ///  - Schedule an image update for newly animating images
    /// Returns whether CSS animations or transitions that style created or updated changed
    /// the style of their elements, so that style is out of date again.
    pub(crate) fn do_post_reflow_update(
        &self,
        cx: &mut JSContext,
        window: &Window,
        now: f64,
    ) -> bool {
        let updates = self.do_post_reflow_update_of_sets(window, now);
        self.css_timing_changed.set(false);
        self.apply_css_animation_updates(cx, window, updates);
        self.handle_animation_presence_or_pending_events_change(window);
        self.css_timing_changed.replace(false)
    }

    /// Note that a CSS animation or transition gave style new timing.
    pub(crate) fn note_css_timing_changed(&self) {
        self.css_timing_changed.set(true);
    }

    /// Create, update and cancel the `CSSAnimation` and `CSSTransition` objects of the
    /// animations and transitions that style generated, updated or canceled.
    fn apply_css_animation_updates(
        &self,
        cx: &mut JSContext,
        window: &Window,
        updates: Vec<CssAnimationUpdate>,
    ) {
        if updates.is_empty() {
            return;
        }
        let owner = |key: &AnimationSetKey| -> Option<DomRoot<Element>> {
            self.rooted_animation_nodes
                .borrow()
                .get(&NoTrace(key.node))
                .and_then(|node| node.downcast::<Element>().map(DomRoot::from_ref))
        };
        let mut new_transitions = false;
        for update in updates {
            match update {
                CssAnimationUpdate::CreateAnimation {
                    key,
                    id,
                    name,
                    name_index,
                    timing,
                    paused,
                } => {
                    let Some(owner) = owner(&key) else {
                        continue;
                    };
                    let animation = DomAnimation::new_css_animation(
                        cx, window, &owner, key, id, &name, name_index, timing, paused,
                    );
                    self.css_animations
                        .borrow_mut()
                        .insert(id, Dom::from_ref(&*animation));
                },
                CssAnimationUpdate::CreateTransition {
                    key,
                    id,
                    property,
                    timing,
                    easing,
                } => {
                    let Some(owner) = owner(&key) else {
                        continue;
                    };
                    if !new_transitions {
                        new_transitions = true;
                        self.transition_generation
                            .set(self.transition_generation.get() + 1);
                    }
                    let animation = DomAnimation::new_css_transition(
                        cx,
                        window,
                        &owner,
                        key,
                        id,
                        &property,
                        self.transition_generation.get(),
                        timing,
                        easing,
                    );
                    self.css_animations
                        .borrow_mut()
                        .insert(id, Dom::from_ref(&*animation));
                },
                CssAnimationUpdate::Update {
                    animation,
                    timing,
                    paused,
                    name_index,
                } => animation.update_from_css_style(cx, timing, paused, name_index),
                CssAnimationUpdate::Cancel(animation) => animation.cancel_from_style(cx),
            }
        }
    }

    fn do_post_reflow_update_of_sets(&self, window: &Window, now: f64) -> Vec<CssAnimationUpdate> {
        let mut sets = self.sets.sets.write();
        {
            let rooted_nodes = self.rooted_animation_nodes.borrow();
            for (key, set) in sets.iter_mut() {
                // Elements inside an SVG element have no box, but they are rendered by it.
                if rooted_nodes.get(&NoTrace(key.node)).is_some_and(|node| {
                    !node.is_being_rendered_or_delegates_rendering(key.pseudo_element) &&
                        !node.downcast::<Element>().is_some_and(|element| {
                            *element.namespace() == ns!(svg) &&
                                node.GetParentElement()
                                    .is_some_and(|parent| *parent.namespace() == ns!(svg))
                        })
                }) {
                    set.cancel_all_animations();
                }
            }
        }

        self.root_newly_animating_dom_nodes(&sets);

        let mut updates = Vec::new();
        {
            let mut css_animations = self.css_animations.borrow_mut();
            for (key, set) in sets.iter_mut() {
                for transition in set.transitions.iter() {
                    match css_animations.get(&transition.id) {
                        Some(animation) if transition.state == AnimationState::Canceled => {
                            updates.push(CssAnimationUpdate::Cancel(animation.as_rooted()));
                        },
                        Some(_) => {},
                        None if transition.state != AnimationState::Canceled &&
                            transition.state != AnimationState::Finished =>
                        {
                            let (timing, easing) = css_transition_timing(transition);
                            updates.push(CssAnimationUpdate::CreateTransition {
                                key: key.clone(),
                                id: transition.id,
                                property: transition.property_animation.property_id().name().into(),
                                timing,
                                easing,
                            });
                        },
                        None => {},
                    }
                }
                for animation in set.animations.iter() {
                    match css_animations.get(&animation.id) {
                        Some(dom_animation) if animation.state == AnimationState::Canceled => {
                            updates.push(CssAnimationUpdate::Cancel(dom_animation.as_rooted()));
                        },
                        Some(dom_animation) => updates.push(CssAnimationUpdate::Update {
                            animation: dom_animation.as_rooted(),
                            timing: css_animation_timing(animation),
                            paused: animation.paused_in_style,
                            name_index: animation.name_index,
                        }),
                        None if animation.state != AnimationState::Canceled => {
                            updates.push(CssAnimationUpdate::CreateAnimation {
                                key: key.clone(),
                                id: animation.id,
                                name: animation.name.to_string(),
                                name_index: animation.name_index,
                                timing: css_animation_timing(animation),
                                paused: animation.paused_in_style,
                            })
                        },
                        None => {},
                    }
                }
            }

            // Forget the CSS animations and transitions that style no longer has, such as
            // completed transitions.
            css_animations.retain(|id, _| {
                sets.values().any(|set| {
                    set.animations.iter().any(|animation| {
                        animation.id == *id && animation.state != AnimationState::Canceled
                    }) || set.transitions.iter().any(|transition| {
                        transition.id == *id && transition.state != AnimationState::Canceled
                    })
                })
            });
        }

        for set in sets.values_mut() {
            self.handle_canceled_animations(set);
            self.handle_new_animations(set);
        }

        // Remove empty states from our collection of states in order to free
        // up space as soon as we are no longer tracking any animations for
        // a node.
        sets.retain(|_, state| !state.is_empty());
        let have_running_animations = sets.values().any(|state| state.needs_animation_ticks());

        self.update_running_animations_presence(window, have_running_animations);

        // Cancel animations for any images that are no longer rendering.
        self.rooted_image_nodes
            .borrow_mut()
            .retain(|opaque_node, node| {
                if node.is_being_rendered(None) {
                    return true;
                }
                self.animating_images.write().remove(opaque_node.0);
                false
            });

        if self.animating_images().write().clear_dirty() {
            self.root_nodes_with_newly_animating_images();
            self.maybe_schedule_image_animation_update(window, now);
        }
        updates
    }

    pub(crate) fn update_active_image_animation_frames(&self, window: &Window, now: f64) {
        if self.animating_images.read().is_empty() {
            return;
        }

        let updates = self
            .animating_images
            .write()
            .node_to_state_map
            .values_mut()
            .filter_map(|state| {
                if !state.update_frame_for_animation_timeline_value(now) {
                    return None;
                }

                let image = &state.image;
                let frame = image
                    .frame_data(state.active_frame)
                    .expect("No frame found")
                    .clone();
                if let Some(mut descriptor) =
                    image.webrender_image_descriptor_and_offset_for_frame()
                {
                    descriptor.offset = frame.byte_range.start as i32;
                    Some(ImageUpdate::UpdateImageForAnimation(
                        image.id.unwrap(),
                        descriptor,
                    ))
                } else {
                    error!("Doing normal image update which will be slow!");
                    None
                }
            })
            .collect();
        window
            .paint_api()
            .update_images(window.webview_id().into(), updates);

        self.maybe_schedule_image_animation_update(window, now);
    }

    fn root_nodes_with_newly_animating_images(&self) {
        let mut rooted_nodes = self.rooted_image_nodes.borrow_mut();
        for opaque_node in self.animating_images.read().node_to_state_map.keys() {
            root_node_and_mark_as_having_animated(&mut rooted_nodes, *opaque_node);
        }
    }

    fn maybe_schedule_image_animation_update(&self, window: &Window, now: f64) {
        with_script_thread(|script_thread| {
            if let Some(current_timer_id) = self.image_callback_timer_id.take() {
                self.image_callback_timer_id.set(None);
                script_thread.cancel_timer(current_timer_id);
            }

            if let Some(duration) = self.duration_to_next_image_animation_frame(now) {
                let trusted_window = Trusted::new(window);
                let timer_id = script_thread.schedule_timer(timers::TimerEventRequest {
                    callback: Box::new(move || {
                        let window = trusted_window.root();
                        window.Document().set_has_pending_animated_image_update();
                    }),
                    duration,
                });
                self.image_callback_timer_id.set(Some(timer_id));
            }
        })
    }

    fn duration_to_next_image_animation_frame(&self, now: f64) -> Option<Duration> {
        self.animating_images
            .read()
            .node_to_state_map
            .values()
            .filter_map(|state| state.duration_to_next_frame(now))
            .min()
    }

    fn update_running_animations_presence(&self, window: &Window, new_value: bool) {
        self.has_running_animations.set(new_value);
        self.handle_animation_presence_or_pending_events_change(window);
    }

    /// Tell the constellation whether this document needs animation ticks: when it has
    /// running animations or animation events to dispatch.
    pub(crate) fn handle_animation_presence_or_pending_events_change(&self, window: &Window) {
        let has_running_animations = self.has_running_animations.get();
        let has_pending_events = self.has_changed_animations.get() ||
            !self.pending_events.borrow().is_empty() ||
            !self.css_events.borrow().is_empty() ||
            !self.pending_playback_events.borrow().is_empty();

        // Do not send the AnimationCallbacksAbsent state until all pending
        // animation events are delivered.
        let present = has_running_animations || has_pending_events;
        if self.animations_present.replace(present) == present {
            return;
        }
        let state = match present {
            true => AnimationsPresentState::AnimationsPresent,
            false => AnimationsPresentState::NoAnimationsPresent,
        };
        window.send_to_constellation(ScriptToConstellationMessage::ChangeRunningAnimationsState(
            state,
        ));
    }

    pub(crate) fn running_animation_count(&self) -> usize {
        self.sets
            .sets
            .read()
            .values()
            .map(|state| state.running_animation_and_transition_count())
            .sum()
    }

    /// Walk through the list of pending animations and start all of the ones that
    /// have left the delay phase.
    fn start_pending_animations(
        &self,
        key: &AnimationSetKey,
        set: &mut ElementAnimationSet,
        now: f64,
        pipeline_id: PipelineId,
    ) {
        for animation in set.animations.iter_mut() {
            if animation.script_timing.is_some() {
                continue;
            }
            if animation.state == AnimationState::Pending && animation.started_at <= now {
                animation.state = AnimationState::Running;
                self.add_animation_event(
                    key,
                    animation,
                    TransitionOrAnimationEventType::AnimationStart,
                    now,
                    pipeline_id,
                );
            }
        }

        for transition in set.transitions.iter_mut() {
            if transition.script_timing.is_some() {
                continue;
            }
            if transition.state == AnimationState::Pending && transition.start_time <= now {
                transition.state = AnimationState::Running;
                self.add_transition_event(
                    key,
                    transition,
                    TransitionOrAnimationEventType::TransitionStart,
                    now,
                    pipeline_id,
                );
            }
        }
    }

    /// Walk through the list of running animations and remove all of the ones that
    /// have ended.
    fn finish_running_animations(
        &self,
        key: &AnimationSetKey,
        set: &mut ElementAnimationSet,
        now: f64,
        pipeline_id: PipelineId,
    ) {
        for animation in set.animations.iter_mut() {
            if animation.script_timing.is_some() {
                continue;
            }
            if animation.state == AnimationState::Running && animation.has_ended(now) {
                animation.state = AnimationState::Finished;
                self.add_animation_event(
                    key,
                    animation,
                    TransitionOrAnimationEventType::AnimationEnd,
                    now,
                    pipeline_id,
                );
            }
        }

        for transition in set.transitions.iter_mut() {
            if transition.script_timing.is_some() {
                continue;
            }
            if transition.state == AnimationState::Running && transition.has_ended(now) {
                transition.state = AnimationState::Finished;
                self.add_transition_event(
                    key,
                    transition,
                    TransitionOrAnimationEventType::TransitionEnd,
                    now,
                    pipeline_id,
                );
            }
        }
    }

    /// Remove the canceled animations and transitions. Their `CSSAnimation` and
    /// `CSSTransition` objects dispatch their events.
    fn handle_canceled_animations(&self, set: &mut ElementAnimationSet) {
        set.clear_canceled_animations();
    }

    fn handle_new_animations(&self, set: &mut ElementAnimationSet) {
        for animation in set.animations.iter_mut() {
            animation.is_new = false;
        }
        for transition in set.transitions.iter_mut() {
            transition.is_new = false;
        }
    }

    /// Ensure that all nodes with new animations are rooted. This should be called
    /// immediately after a restyle, to ensure that these addresses are still valid.
    fn root_newly_animating_dom_nodes(
        &self,
        sets: &style::FxHashMap<AnimationSetKey, ElementAnimationSet>,
    ) {
        let mut rooted_nodes = self.rooted_animation_nodes.borrow_mut();
        for (key, set) in sets.iter() {
            let opaque_node = key.node;
            if rooted_nodes.contains_key(&NoTrace(opaque_node)) {
                continue;
            }

            if set.animations.iter().any(|animation| animation.is_new) ||
                set.transitions.iter().any(|transition| transition.is_new)
            {
                root_node_and_mark_as_having_animated(&mut rooted_nodes, opaque_node);
            }
        }
    }

    // Unroot any nodes that we have rooted but are no longer tracking animations for.
    fn unroot_unused_nodes(&self, sets: &style::FxHashMap<AnimationSetKey, ElementAnimationSet>) {
        let pending_events = self.pending_events.borrow();
        let nodes: style::FxHashSet<OpaqueNode> = sets.keys().map(|key| key.node).collect();
        self.rooted_animation_nodes.borrow_mut().retain(|node, _| {
            nodes.contains(&node.0) || pending_events.iter().any(|event| event.node == node.0)
        });
    }

    fn add_transition_event(
        &self,
        key: &AnimationSetKey,
        transition: &Transition,
        event_type: TransitionOrAnimationEventType,
        now: f64,
        pipeline_id: PipelineId,
    ) {
        // Calculate the `elapsed-time` property of the event and take the absolute
        // value to prevent -0 values.
        let elapsed_time = match event_type {
            TransitionOrAnimationEventType::TransitionRun |
            TransitionOrAnimationEventType::TransitionStart => transition
                .property_animation
                .duration
                .min((-transition.delay).max(0.)),
            TransitionOrAnimationEventType::TransitionEnd => transition.property_animation.duration,
            TransitionOrAnimationEventType::TransitionCancel => {
                (now - transition.start_time).max(0.)
            },
            _ => unreachable!(),
        }
        .abs();

        self.pending_events
            .borrow_mut()
            .push(TransitionOrAnimationEvent {
                pipeline_id,
                event_type,
                node: key.node,
                pseudo_element: key.pseudo_element,
                property_or_animation_name: transition
                    .property_animation
                    .property_id()
                    .name()
                    .into(),
                elapsed_time,
            });
    }

    fn add_animation_event(
        &self,
        key: &AnimationSetKey,
        animation: &Animation,
        event_type: TransitionOrAnimationEventType,
        now: f64,
        pipeline_id: PipelineId,
    ) {
        let iteration_index = match animation.iteration_state {
            KeyframesIterationState::Finite(current, _) |
            KeyframesIterationState::Infinite(current) => current,
        };

        let active_duration = match animation.iteration_state {
            KeyframesIterationState::Finite(_, max) => max * animation.duration,
            KeyframesIterationState::Infinite(_) => f64::MAX,
        };

        // Calculate the `elapsed-time` property of the event and take the absolute
        // value to prevent -0 values.
        let elapsed_time = match event_type {
            TransitionOrAnimationEventType::AnimationStart => {
                (-animation.delay).max(0.).min(active_duration)
            },
            TransitionOrAnimationEventType::AnimationIteration => {
                iteration_index * animation.duration
            },
            TransitionOrAnimationEventType::AnimationEnd => {
                (iteration_index * animation.duration) + animation.current_iteration_duration()
            },
            TransitionOrAnimationEventType::AnimationCancel => {
                (iteration_index * animation.duration) + (now - animation.started_at).max(0.)
            },
            _ => unreachable!(),
        }
        .abs();

        self.pending_events
            .borrow_mut()
            .push(TransitionOrAnimationEvent {
                pipeline_id,
                event_type,
                node: key.node,
                pseudo_element: key.pseudo_element,
                property_or_animation_name: animation.name.to_string(),
                elapsed_time,
            });
    }

    /// An implementation of the final steps of
    /// <https://drafts.csswg.org/web-animations-1/#update-animations-and-send-events>.
    pub(crate) fn send_pending_events(&self, window: &Window, cx: &mut js::context::JSContext) {
        // > 4. Let events to dispatch be a copy of doc’s pending animation event queue.
        // > 5. Clear doc’s pending animation event queue.
        //
        // Snapshot every queue before dispatch. A listener may queue another event, which
        // belongs to the next update rather than this frame.
        let playback_events: Vec<PlaybackEventToDispatch> = self
            .pending_playback_events
            .borrow_mut()
            .drain(..)
            .map(|event| PlaybackEventToDispatch {
                animation: event.animation.as_rooted(),
                event_type: event.event_type,
                current_time: event.current_time,
                timeline_time: event.timeline_time,
                scheduled_event_time: event.scheduled_event_time,
                target: event.target.map(|target| target.as_rooted()),
                order: event.order,
            })
            .collect();
        let css_events: Vec<CssEventToDispatch> = self
            .css_events
            .borrow_mut()
            .drain(..)
            .map(|event| {
                (
                    event.animation.as_rooted(),
                    event.target.as_rooted(),
                    event.pseudo_element,
                    event.event_type,
                    event.name,
                    event.elapsed_time,
                    event.scheduled_event_time,
                    event.order,
                )
            })
            .collect();
        let events = std::mem::take(&mut *self.pending_events.safe_borrow_mut(cx.no_gc()));
        self.send_playback_events(window, cx, playback_events);
        self.send_css_events(cx, css_events);
        if events.is_empty() {
            self.handle_animation_presence_or_pending_events_change(window);
            return;
        }

        // > 6. Perform a stable sort of the animation events in events to dispatch as follows:
        // >    1. Sort the events by their scheduled event time such that events that were
        // >       scheduled to occur earlier sort before events scheduled to occur later, and
        // >       events whose scheduled event time is unresolved sort before events with a
        // >       resolved scheduled event time.
        // >    2. Within events with equal scheduled event times, sort by their composite
        // >       order.
        //
        // TODO: Sorting of animation events isn't done yet.

        // 7. Dispatch each of the events in events to dispatch at their corresponding
        // target using the order established in the previous step.
        for event in events.into_iter() {
            // We root the node here to ensure that sending this event doesn't
            // unroot it as a side-effect.
            let node = match self
                .rooted_animation_nodes
                .borrow()
                .get(&NoTrace(event.node))
            {
                Some(node) => DomRoot::from_ref(&**node),
                None => {
                    warn!("Tried to send an event for an unrooted node");
                    continue;
                },
            };

            let event_atom = match event.event_type {
                TransitionOrAnimationEventType::AnimationEnd => atom!("animationend"),
                TransitionOrAnimationEventType::AnimationStart => atom!("animationstart"),
                TransitionOrAnimationEventType::AnimationCancel => atom!("animationcancel"),
                TransitionOrAnimationEventType::AnimationIteration => atom!("animationiteration"),
                TransitionOrAnimationEventType::TransitionCancel => atom!("transitioncancel"),
                TransitionOrAnimationEventType::TransitionEnd => atom!("transitionend"),
                TransitionOrAnimationEventType::TransitionRun => atom!("transitionrun"),
                TransitionOrAnimationEventType::TransitionStart => atom!("transitionstart"),
            };
            let parent = EventInit {
                bubbles: true,
                cancelable: false,
                composed: false,
            };

            let property_or_animation_name =
                DOMString::from(event.property_or_animation_name.clone());
            let pseudo_element = event
                .pseudo_element
                .map_or_else(DOMString::new, |pseudo_element| {
                    DOMString::from(pseudo_element.to_css_string())
                });
            let elapsed_time = Finite::new(event.elapsed_time as f32).unwrap();
            let window = node.owner_window();

            if event.event_type.is_transition_event() {
                let event_init = TransitionEventInit {
                    parent,
                    propertyName: property_or_animation_name,
                    elapsedTime: elapsed_time,
                    pseudoElement: pseudo_element,
                };
                TransitionEvent::new(cx, &window, event_atom, &event_init)
                    .upcast::<Event>()
                    .fire(cx, node.upcast());
            } else {
                let event_init = AnimationEventInit {
                    parent,
                    animationName: property_or_animation_name,
                    elapsedTime: elapsed_time,
                    pseudoElement: pseudo_element,
                };
                AnimationEvent::new(cx, &window, event_atom, &event_init)
                    .upcast::<Event>()
                    .fire(cx, node.upcast());
            }
        }

        self.handle_animation_presence_or_pending_events_change(window);
    }

    /// Dispatch the queued events of CSS animations and transitions, sorted in the
    /// composite order of their animations.
    fn send_css_events(
        &self,
        cx: &mut js::context::JSContext,
        mut events: Vec<CssEventToDispatch>,
    ) {
        if events.is_empty() {
            return;
        }
        {
            let no_gc = cx.no_gc();
            // > Sort the events by their scheduled event time such that events that were
            // > scheduled to occur earlier sort before events scheduled to occur later, and
            // > events whose scheduled event time is unresolved sort before events with a
            // > resolved scheduled event time. Within events with equal scheduled event
            // > times, sort by their composite order.
            events.sort_by(|a, b| {
                let time_order = match (a.6, b.6) {
                    (None, None) => std::cmp::Ordering::Equal,
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal),
                };
                // Canceled animations sort before animations that have not been canceled.
                let is_cancel = |event_type: TransitionOrAnimationEventType| {
                    matches!(
                        event_type,
                        TransitionOrAnimationEventType::AnimationCancel |
                            TransitionOrAnimationEventType::TransitionCancel
                    )
                };
                // Then sort by composite order: transitions before animations, then by the
                // tree order of their owning elements, then by their order on the element.
                time_order
                    .then_with(|| is_cancel(b.3).cmp(&is_cancel(a.3)))
                    .then_with(|| b.7.is_transition().cmp(&a.7.is_transition()))
                    .then_with(|| {
                        if a.1 == b.1 {
                            a.7.cmp_same_target(&b.7)
                        } else if a.1.upcast::<Node>().is_before(no_gc, b.1.upcast()) {
                            std::cmp::Ordering::Less
                        } else {
                            std::cmp::Ordering::Greater
                        }
                    })
            });
        }
        for (animation, target, pseudo_element, event_type, name, elapsed_time, _, _) in events {
            let event_atom = match event_type {
                TransitionOrAnimationEventType::AnimationEnd => atom!("animationend"),
                TransitionOrAnimationEventType::AnimationStart => atom!("animationstart"),
                TransitionOrAnimationEventType::AnimationCancel => atom!("animationcancel"),
                TransitionOrAnimationEventType::AnimationIteration => atom!("animationiteration"),
                TransitionOrAnimationEventType::TransitionCancel => atom!("transitioncancel"),
                TransitionOrAnimationEventType::TransitionEnd => atom!("transitionend"),
                TransitionOrAnimationEventType::TransitionRun => atom!("transitionrun"),
                TransitionOrAnimationEventType::TransitionStart => atom!("transitionstart"),
            };
            let parent = EventInit {
                bubbles: true,
                cancelable: false,
                composed: false,
            };
            let name = DOMString::from(name);
            let pseudo_element = pseudo_element.map_or_else(DOMString::new, |pseudo_element| {
                DOMString::from(pseudo_element.to_css_string())
            });
            let elapsed_time = Finite::new(elapsed_time as f32).unwrap_or(Finite::wrap(0.0));
            let window = target.owner_window();
            if event_type.is_transition_event() {
                let event_init = TransitionEventInit {
                    parent,
                    propertyName: name,
                    elapsedTime: elapsed_time,
                    pseudoElement: pseudo_element,
                };
                let event = TransitionEvent::new(cx, &window, event_atom, &event_init);
                event.set_animation(&animation);
                event.upcast::<Event>().fire(cx, target.upcast());
            } else {
                let event_init = AnimationEventInit {
                    parent,
                    animationName: name,
                    elapsedTime: elapsed_time,
                    pseudoElement: pseudo_element,
                };
                let event = AnimationEvent::new(cx, &window, event_atom, &event_init);
                event.set_animation(&animation);
                event.upcast::<Event>().fire(cx, target.upcast());
            }
        }
    }
}

/// The type of transition event to trigger. These are defined by
/// CSS Transitions § 6.1 and CSS Animations § 4.2
#[derive(Clone, Copy, Debug, Deserialize, JSTraceable, MallocSizeOf, PartialEq, Serialize)]
pub(crate) enum TransitionOrAnimationEventType {
    /// "The transitionrun event occurs when a transition is created (i.e., when it
    /// is added to the set of running transitions)."
    TransitionRun,
    /// "The transitionstart event occurs when a transition’s delay phase ends."
    TransitionStart,
    /// "The transitionend event occurs at the completion of the transition. In the
    /// case where a transition is removed before completion, such as if the
    /// transition-property is removed, then the event will not fire."
    TransitionEnd,
    /// "The transitioncancel event occurs when a transition is canceled."
    TransitionCancel,
    /// "The animationstart event occurs at the start of the animation. If there is
    /// an animation-delay then this event will fire once the delay period has expired."
    AnimationStart,
    /// "The animationiteration event occurs at the end of each iteration of an
    /// animation, except when an animationend event would fire at the same time."
    AnimationIteration,
    /// "The animationend event occurs when the animation finishes"
    AnimationEnd,
    /// "The animationcancel event occurs when the animation stops running in a way
    /// that does not fire an animationend event..."
    AnimationCancel,
}

impl TransitionOrAnimationEventType {
    /// Whether or not this event is a transition-related event.
    pub(crate) fn is_transition_event(&self) -> bool {
        match *self {
            Self::TransitionRun |
            Self::TransitionEnd |
            Self::TransitionCancel |
            Self::TransitionStart => true,
            Self::AnimationEnd |
            Self::AnimationIteration |
            Self::AnimationStart |
            Self::AnimationCancel => false,
        }
    }
}

#[derive(Deserialize, JSTraceable, MallocSizeOf, Serialize)]
/// A transition or animation event.
pub(crate) struct TransitionOrAnimationEvent {
    /// The pipeline id of the layout task that sent this message.
    #[no_trace]
    pub(crate) pipeline_id: PipelineId,
    /// The type of transition event this should trigger.
    pub(crate) event_type: TransitionOrAnimationEventType,
    /// The address of the node which owns this transition.
    #[no_trace]
    pub(crate) node: OpaqueNode,
    /// The pseudo element for this transition or animation, if applicable.
    #[no_trace]
    pub(crate) pseudo_element: Option<PseudoElement>,
    /// The name of the property that is transitioning (in the case of a transition)
    /// or the name of the animation (in the case of an animation).
    pub(crate) property_or_animation_name: String,
    /// The elapsed time property to send with this transition event.
    pub(crate) elapsed_time: f64,
}

/// Inserts [opaque_node] into [rooted_nodes], if it isn't already present. This should be
/// called immediately after a restyle, to ensure that these addresses are still valid.
fn root_node_and_mark_as_having_animated(
    rooted_nodes: &mut FxHashMap<NoTrace<OpaqueNode>, Dom<Node>>,
    opaque_node: OpaqueNode,
) {
    #[expect(unsafe_code)]
    rooted_nodes.entry(NoTrace(opaque_node)).or_insert_with(|| {
        // SAFETY: This should be safe as this method is run directly after layout,
        // which should not remove any nodes.
        let address = UntrustedNodeAddress(opaque_node.0 as *const c_void);
        let node = unsafe { DomRoot::from_ref(&*from_untrusted_node_address(address)) };
        node.set_flag(NodeFlags::HAS_EVER_ANIMATED, true);
        node.as_traced()
    });
}
