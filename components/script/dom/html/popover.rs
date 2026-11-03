/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The popover API: <https://html.spec.whatwg.org/multipage/#the-popover-attribute>

use std::cell::Cell;
use std::rc::Rc;

use html5ever::{local_name, ns};
use js::context::{JSContext, NoGC};
use script_bindings::root::Dom;
use servo_config::pref;
use stylo_atoms::Atom;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::HTMLElementBinding::ShowPopoverOptions;
use crate::dom::bindings::codegen::Bindings::NodeBinding::{GetRootNodeOptions, NodeMethods};
use crate::dom::bindings::codegen::UnionTypes::TogglePopoverOptionsOrBoolean;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::refcounted::Trusted;
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::closewatcher::internalclosewatcher::{
    InternalCloseWatcher, InternalCloseWatcherOwner,
};
use crate::dom::document::Document;
use crate::dom::documentfragment::DocumentFragment;
use crate::dom::element::Element;
use crate::dom::event::{Event, EventBubbles, EventCancelable};
use crate::dom::eventtarget::EventTarget;
use crate::dom::html::form_controls::htmlbuttonelement::HTMLButtonElement;
use crate::dom::html::form_controls::htmlinputelement::HTMLInputElement;
use crate::dom::html::form_controls::input_type::InputType;
use crate::dom::html::htmlelement::HTMLElement;
use crate::dom::html::htmlformelement::FormControl;
use crate::dom::html::interactive::htmldialogelement::HTMLDialogElement;
use crate::dom::iterators::ShadowIncluding;
use crate::dom::node::focus::FocusTrigger;
use crate::dom::node::{Node, NodeTraits};
use crate::dom::shadowroot::shadowroot::ShadowRoot;
use crate::dom::toggleevent::ToggleEvent;

/// The states of the `popover` content attribute.
///
/// <https://html.spec.whatwg.org/multipage/#attr-popover>
#[derive(Clone, Copy, Debug, Default, Eq, JSTraceable, MallocSizeOf, PartialEq)]
pub(crate) enum PopoverState {
    /// <https://html.spec.whatwg.org/multipage/#attr-popover-none-state>
    #[default]
    NoPopover,
    /// <https://html.spec.whatwg.org/multipage/#attr-popover-auto-state>
    Auto,
    /// <https://html.spec.whatwg.org/multipage/#attr-popover-manual-state>
    Manual,
    /// <https://html.spec.whatwg.org/multipage/#attr-popover-hint-state>
    Hint,
}

impl PopoverState {
    /// Returns the state of the `popover` content attribute of `element`.
    fn from_attribute(element: &Element) -> PopoverState {
        let Some(value) = element.get_attribute_string_ref(&local_name!("popover")) else {
            // > The attribute's missing value default is the No Popover state,
            return PopoverState::NoPopover;
        };
        // > its empty value default is the Auto state.
        if value.is_empty() || value.eq_ignore_ascii_case("auto") {
            PopoverState::Auto
        } else if value.eq_ignore_ascii_case("hint") {
            PopoverState::Hint
        } else {
            // > its invalid value default is the Manual state,
            PopoverState::Manual
        }
    }
}

/// The values of <https://html.spec.whatwg.org/multipage/#opened-in-popover-mode>, which are
/// also the two kinds of popover stacks of a document.
#[derive(Clone, Copy, Debug, Eq, JSTraceable, MallocSizeOf, PartialEq)]
pub(crate) enum PopoverMode {
    Auto,
    Hint,
}

/// The `oldState` and `newState` values of a popover's toggle events.
#[derive(Clone, Copy, Debug, Eq, JSTraceable, MallocSizeOf, PartialEq)]
enum PopoverToggleState {
    Closed,
    Open,
}

impl PopoverToggleState {
    fn as_str(self) -> &'static str {
        match self {
            PopoverToggleState::Closed => "closed",
            PopoverToggleState::Open => "open",
        }
    }
}

/// The popover-related state of an HTML element. The popover visibility state is kept in the
/// element's [`ElementState::POPOVER_OPEN`] flag instead, which is what `:popover-open` matches.
#[derive(Default, JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct PopoverData {
    /// The state of the `popover` attribute, as last seen by the attribute change steps.
    ///
    /// The popover algorithms use this cached state rather than the content attribute, so that
    /// a popover whose `popover` attribute was just removed can still be hidden by the attribute
    /// change steps. This matches other engines.
    state: PopoverState,
    /// <https://html.spec.whatwg.org/multipage/#popover-trigger>
    trigger: Option<Dom<HTMLElement>>,
    /// <https://html.spec.whatwg.org/multipage/#popover-hiding>
    hiding: bool,
    /// <https://html.spec.whatwg.org/multipage/#popover-toggle-task-tracker>
    ///
    /// Only the old state is stored here. A queued toggle task cannot be removed from its task
    /// queue, so instead every task records the value of `toggle_task_generation` at the time it
    /// was queued, and does nothing if a later task has been queued since.
    toggle_task_tracker: Option<PopoverToggleState>,
    toggle_task_generation: u32,
    /// <https://html.spec.whatwg.org/multipage/#popover-close-watcher>
    #[conditional_malloc_size_of]
    close_watcher: Option<Rc<InternalCloseWatcher>>,
    /// <https://html.spec.whatwg.org/multipage/#opened-in-popover-mode>
    opened_in_popover_mode: Option<PopoverMode>,
}

/// The popover-related state of a document.
#[derive(Default, JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct DocumentPopoverData {
    /// <https://html.spec.whatwg.org/multipage/#popover-pointerdown-target>
    pointerdown_target: MutNullableDom<HTMLElement>,
    /// <https://html.spec.whatwg.org/multipage/#hiding-popover-nesting-count>
    hiding_popover_nesting_count: Cell<u32>,
    /// <https://html.spec.whatwg.org/multipage/#showing-popover>
    showing_popover: Cell<bool>,
    /// <https://html.spec.whatwg.org/multipage/#hint-stack-parent>
    hint_stack_parent: MutNullableDom<HTMLElement>,
}

impl HTMLElement {
    /// Returns the state of this element's `popover` attribute.
    pub(crate) fn popover_state(&self) -> PopoverState {
        self.upcast::<Element>()
            .popover_data()
            .map_or(PopoverState::NoPopover, |data| data.state)
    }

    /// Returns whether this element's popover visibility state is showing.
    ///
    /// <https://html.spec.whatwg.org/multipage/#popover-visibility-state>
    pub(crate) fn is_popover_showing(&self) -> bool {
        self.upcast::<Element>()
            .state()
            .contains(ElementState::POPOVER_OPEN)
    }

    /// <https://html.spec.whatwg.org/multipage/#opened-in-popover-mode>
    fn opened_in_popover_mode(&self) -> Option<PopoverMode> {
        self.upcast::<Element>()
            .popover_data()
            .and_then(|data| data.opened_in_popover_mode)
    }

    /// <https://html.spec.whatwg.org/multipage/#popover-trigger>
    pub(crate) fn popover_trigger(&self) -> Option<DomRoot<HTMLElement>> {
        self.upcast::<Element>()
            .popover_data()
            .and_then(|data| data.trigger.as_ref().map(|trigger| trigger.as_rooted()))
    }

    fn with_popover_data_mut<R>(&self, no_gc: &NoGC, f: impl FnOnce(&mut PopoverData) -> R) -> R {
        f(&mut self
            .upcast::<Element>()
            .ensure_rare_data(no_gc)
            .popover_data)
    }

    /// The attribute change steps for the `popover` attribute.
    ///
    /// <https://html.spec.whatwg.org/multipage/#the-popover-attribute:concept-element-attributes-change-ext>
    pub(crate) fn popover_attribute_changed(&self, cx: &mut JSContext) {
        if !pref!(dom_popover_enabled) {
            return;
        }

        // Step 3. If element's popover visibility state is in the showing state and oldValue and
        // value are in different states, then run the hide popover algorithm given element, true,
        // true, false, and null.
        let mut new_state = PopoverState::from_attribute(self.upcast::<Element>());
        if new_state == self.popover_state() {
            return;
        }
        if self.is_popover_showing() {
            let _ = self.hide_popover(cx, true, true, false, None);
            // Event handlers might have changed the attribute again while the popover was hiding.
            new_state = PopoverState::from_attribute(self.upcast::<Element>());
        }

        if new_state != PopoverState::NoPopover || self.upcast::<Element>().popover_data().is_some()
        {
            self.with_popover_data_mut(cx.no_gc(), |data| data.state = new_state);
        }
    }

    /// The part of the HTML node removing steps that concerns popovers.
    ///
    /// <https://html.spec.whatwg.org/multipage/#dom-trees:concept-node-remove-ext>
    pub(crate) fn popover_removing_steps(&self, cx: &mut JSContext) {
        // Step 5. If removedNode's popover attribute is not in the No Popover state, then run the
        // hide popover algorithm given removedNode, false, false, false, and null.
        if self.popover_state() != PopoverState::NoPopover {
            let _ = self.hide_popover(cx, false, false, false, None);
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-popover>
    pub(crate) fn popover_reflected_value(&self) -> Option<DOMString> {
        // The attribute is limited to only known values, and its missing value default, No
        // Popover, has no keyword.
        let element = self.upcast::<Element>();
        if !element.has_attribute(&local_name!("popover")) {
            return None;
        }
        Some(DOMString::from_static(
            match PopoverState::from_attribute(element) {
                PopoverState::Auto => "auto",
                PopoverState::Hint => "hint",
                PopoverState::Manual | PopoverState::NoPopover => "manual",
            },
        ))
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-showpopover>
    pub(crate) fn show_popover_method(
        &self,
        cx: &mut JSContext,
        options: &ShowPopoverOptions,
    ) -> ErrorResult {
        // Step 1. Let source be options["source"] if it exists; otherwise, null.
        // Step 2. Run show popover given this, true, and source.
        self.show_popover(cx, true, options.source.as_deref())
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hidepopover>
    pub(crate) fn hide_popover_method(&self, cx: &mut JSContext) -> ErrorResult {
        // Run the hide popover algorithm given this, true, true, true, and null.
        self.hide_popover(cx, true, true, true, None)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-togglepopover>
    pub(crate) fn toggle_popover_method(
        &self,
        cx: &mut JSContext,
        options: &TogglePopoverOptionsOrBoolean,
    ) -> Fallible<bool> {
        // Step 1. Let force be null.
        // Step 2. If options is a boolean, set force to options.
        // Step 3. Otherwise, if options["force"] exists, set force to options["force"].
        // Step 4. Let source be options["source"] if it exists; otherwise, null.
        let (force, source) = match options {
            TogglePopoverOptionsOrBoolean::Boolean(force) => (Some(*force), None),
            TogglePopoverOptionsOrBoolean::TogglePopoverOptions(options) => {
                (options.force, options.parent.source.as_deref())
            },
        };

        // Step 5. If this's popover visibility state is showing, and force is null or false, then
        // run the hide popover algorithm given this, true, true, true, and null.
        if self.is_popover_showing() && force != Some(true) {
            self.hide_popover(cx, true, true, true, None)?;
        }
        // Step 6. Otherwise, if force is null or true, then run show popover given this, true,
        // and source.
        else if force != Some(false) {
            self.show_popover(cx, true, source)?;
        }
        // Step 7. Otherwise:
        else {
            // Step 7.1. Let expectedToBeShowing be true if this's popover visibility state is
            // showing; otherwise false.
            let expected_to_be_showing = self.is_popover_showing();
            // Step 7.2. Run check popover validity given this, expectedToBeShowing, and null.
            self.check_popover_validity(expected_to_be_showing, None)?;
        }

        // Step 8. Return true if this's popover visibility state is showing; otherwise false.
        Ok(self.is_popover_showing())
    }

    /// <https://html.spec.whatwg.org/multipage/#check-popover-validity>
    pub(crate) fn check_popover_validity(
        &self,
        expected_to_be_showing: bool,
        expected_document: Option<&Document>,
    ) -> Fallible<bool> {
        // Step 1. If element's popover attribute is in the No Popover state, then throw a
        // "NotSupportedError" DOMException.
        if self.popover_state() == PopoverState::NoPopover {
            return Err(Error::NotSupported(Some(
                "Element does not have a popover attribute".into(),
            )));
        }

        // Step 2. If any of the following are true:
        // - expectedToBeShowing is true and element's popover visibility state is not showing; or
        // - expectedToBeShowing is false and element's popover visibility state is not hidden,
        // then return false.
        if expected_to_be_showing != self.is_popover_showing() {
            return Ok(false);
        }

        // Step 3. If any of the following are true:
        // - expectedToBeShowing is false and element is not connected;
        // - expectedToBeShowing is false and element's node document is not fully active;
        // - expectedDocument is not null and element's node document is not expectedDocument;
        // - element is a dialog element and its is modal is set to true; or
        // - element's fullscreen flag is set,
        // then throw an "InvalidStateError" DOMException.
        let element = self.upcast::<Element>();
        let document = element.owner_document();
        if (!expected_to_be_showing && (!element.is_connected() || !document.is_fully_active())) ||
            expected_document.is_some_and(|expected_document| *expected_document != *document) ||
            (self.is::<HTMLDialogElement>() && element.state().contains(ElementState::MODAL)) ||
            element.state().contains(ElementState::FULLSCREEN)
        {
            return Err(Error::InvalidState(Some(
                "Element is not in a valid state to change its popover visibility".into(),
            )));
        }

        // Step 4. Return true.
        Ok(true)
    }

    /// <https://html.spec.whatwg.org/multipage/#show-popover>
    pub(crate) fn show_popover(
        &self,
        cx: &mut JSContext,
        throw_exceptions: bool,
        source: Option<&HTMLElement>,
    ) -> ErrorResult {
        // Step 1. Let document be element's node document.
        let document = self.owner_document();
        let popovers = document.popover_data();

        // Step 2. If document's showing popover is true, or document's hiding popover nesting
        // count is not 0:
        if popovers.showing_popover.get() || popovers.hiding_popover_nesting_count.get() != 0 {
            // Step 2.1. If throwExceptions is true, then throw a "InvalidStateError" DOMException.
            if throw_exceptions {
                return Err(Error::InvalidState(Some(
                    "Cannot show a popover while another popover is being shown or hidden".into(),
                )));
            }
            // Step 2.2. Return.
            return Ok(());
        }

        // Step 3. Let validityResult be the result of running check popover validity given
        // element, false, and null. If this throws an exception, catch it, and set validityResult
        // to that exception.
        // Step 4. If validityResult is not true:
        // Step 4.1. If throwExceptions is true and validityResult is a DOMException, then throw
        // validityResult.
        // Step 4.2. Return.
        if !self
            .handle_popover_validity(self.check_popover_validity(false, None), throw_exceptions)?
        {
            return Ok(());
        }

        // Step 5. Assert: element's popover trigger is null.
        debug_assert!(self.popover_trigger().is_none());

        // Step 6. Assert: element is not in document's top layer.
        debug_assert!(!document.top_layer_contains(self.upcast::<Element>()));

        // Step 7. Set document's showing popover to true.
        popovers.showing_popover.set(true);

        // Step 8. Let cleanupShowingSteps be the following steps:
        // Step 8.1. Set document's showing popover to false.
        let cleanup_showing_steps = || popovers.showing_popover.set(false);

        // Step 9. If the result of firing an event named beforetoggle, using ToggleEvent, with the
        // cancelable attribute initialized to true, the oldState attribute initialized to
        // "closed", the newState attribute initialized to "open", and the source attribute
        // initialized to source at element is false, then run cleanupShowingSteps and return.
        let event = ToggleEvent::new(
            cx,
            &self.owner_window(),
            atom!("beforetoggle"),
            EventBubbles::DoesNotBubble,
            EventCancelable::Cancelable,
            DOMString::from_static("closed"),
            DOMString::from_static("open"),
            source.map(|source| source.upcast::<Element>()),
        );
        if !event
            .upcast::<Event>()
            .fire(cx, self.upcast::<EventTarget>())
        {
            cleanup_showing_steps();
            return Ok(());
        }

        // Step 10. Set validityResult to the result of running check popover validity given
        // element, false, and document. If this throws an exception, catch it, and set
        // validityResult to that exception.
        // Step 11. If validityResult is not true:
        // Step 11.1. Run cleanupShowingSteps.
        // Step 11.2. If throwExceptions is true and validityResult is a DOMException, then throw
        // validityResult.
        // Step 11.3. Return.
        let validity = self.check_popover_validity(false, Some(&document));
        if !self
            .handle_popover_validity(validity, throw_exceptions)
            .inspect_err(|_| cleanup_showing_steps())?
        {
            cleanup_showing_steps();
            return Ok(());
        }

        // Step 12. Let shouldRestoreFocus be false.
        let mut should_restore_focus = false;

        // Step 13. Let originalType be the current state of element's popover attribute.
        let original_type = self.popover_state();

        // Step 14. Let effectiveType be originalType.
        let mut effective_type = original_type;

        // Step 15. If originalType is Auto or Hint:
        let mut ancestor = None;
        if matches!(original_type, PopoverState::Auto | PopoverState::Hint) {
            // Step 15.1. Let ancestor be the result of running the topmost popover ancestor
            // algorithm given element, source, and true.
            ancestor = topmost_popover_ancestor(cx.no_gc(), self.upcast(), source, true);

            // Step 15.2. If all of the following are true:
            // - ancestor is not null;
            // - ancestor's opened in popover mode is "hint"; and
            // - effectiveType is the Auto state,
            // then set effectiveType to the Hint state.
            if ancestor.as_ref().is_some_and(|ancestor| {
                ancestor.opened_in_popover_mode() == Some(PopoverMode::Hint)
            }) && effective_type == PopoverState::Auto
            {
                effective_type = PopoverState::Hint;
            }

            // Step 15.3. Run hide popover stack until given document, ancestor, Hint,
            // shouldRestoreFocus, and true.
            document.hide_popover_stack_until(
                cx,
                ancestor.as_deref(),
                PopoverMode::Hint,
                should_restore_focus,
                true,
            );

            // Step 15.4. If effectiveType is the Auto state, then run hide popover stack until
            // given document, ancestor, Auto, shouldRestoreFocus, and true.
            if effective_type == PopoverState::Auto {
                document.hide_popover_stack_until(
                    cx,
                    ancestor.as_deref(),
                    PopoverMode::Auto,
                    should_restore_focus,
                    true,
                );
            }

            // Step 15.5. If originalType is not equal to the value of element's popover attribute:
            if original_type != self.popover_state() {
                // Step 15.5.1. Run cleanupShowingSteps.
                cleanup_showing_steps();
                // Step 15.5.2. If throwExceptions is true, then throw an "InvalidStateError"
                // DOMException.
                if throw_exceptions {
                    return Err(Error::InvalidState(Some(
                        "The popover attribute changed while showing the popover".into(),
                    )));
                }
                // Step 15.5.3. Return.
                return Ok(());
            }

            // Step 15.6. Set validityResult to the result of running check popover validity given
            // element, false, and document. If this throws an exception, catch it, and set
            // validityResult to that exception.
            // Step 15.7. If validityResult is not true:
            // Step 15.7.1. Run cleanupShowingSteps.
            // Step 15.7.2. If throwExceptions is true and validityResult is a DOMException, then
            // throw validityResult.
            // Step 15.7.3. Return.
            let validity = self.check_popover_validity(false, Some(&document));
            if !self
                .handle_popover_validity(validity, throw_exceptions)
                .inspect_err(|_| cleanup_showing_steps())?
            {
                cleanup_showing_steps();
                return Ok(());
            }

            // Step 15.8. If the result of running topmost auto or hint popover on document is
            // null, then set shouldRestoreFocus to true.
            if document.topmost_auto_or_hint_popover().is_none() {
                should_restore_focus = true;
            }

            // Step 15.9. If effectiveType is Auto:
            // Step 15.9.1. Assert: document's showing auto popover list does not contain element.
            // Step 15.9.2. Set element's opened in popover mode to "auto".
            // Step 15.10. Otherwise:
            // Step 15.10.1. Assert: effectiveType is Hint.
            // Step 15.10.2. Assert: document's showing hint popover list does not contain element.
            // Step 15.10.3. Set element's opened in popover mode to "hint".
            let mode = if effective_type == PopoverState::Auto {
                PopoverMode::Auto
            } else {
                debug_assert_eq!(effective_type, PopoverState::Hint);
                PopoverMode::Hint
            };
            debug_assert!(
                !document
                    .showing_popover_list(mode)
                    .iter()
                    .any(|popover| &**popover == self)
            );

            // Step 15.11. Set element's popover close watcher to the result of establishing a
            // close watcher given element's relevant global object, with:
            // - cancelAction being to return true.
            // - closeAction being to hide a popover given element, true, true, false, and null.
            // - getEnabledState being to return true.
            let close_watcher = InternalCloseWatcher::establish(
                cx.no_gc(),
                &self.owner_window(),
                &InternalCloseWatcherOwner::Popover(Dom::from_ref(self)),
            );
            self.with_popover_data_mut(cx.no_gc(), |data| {
                data.opened_in_popover_mode = Some(mode);
                data.close_watcher = Some(close_watcher);
            });
        }

        // Step 16. Set element's previously focused element to null.
        self.set_previously_focused_element(None, cx.no_gc());

        // Step 17. Let originallyFocusedElement be document's focused area of the document's DOM
        // anchor.
        let originally_focused_element = document
            .focus_handler()
            .focused_area()
            .element()
            .map(DomRoot::from_ref);

        // Step 18. Add an element to the top layer given element.
        document.add_an_element_to_the_top_layer(cx.no_gc(), self.upcast::<Element>());

        // Step 19. If effectiveType is Hint and ancestor's opened in popover mode is "auto", then
        // set document's hint stack parent to ancestor.
        if effective_type == PopoverState::Hint {
            if let Some(ancestor) = ancestor.as_deref() {
                if ancestor.opened_in_popover_mode() == Some(PopoverMode::Auto) {
                    popovers.hint_stack_parent.set(Some(ancestor));
                }
            }
        }

        // Step 20. Set element's popover visibility state to showing.
        self.upcast::<Element>()
            .set_state(cx.no_gc(), ElementState::POPOVER_OPEN, true);

        // Step 21. Set element's popover trigger to source.
        self.with_popover_data_mut(cx.no_gc(), |data| {
            data.trigger = source.map(Dom::from_ref);
        });

        // TODO: Step 22. Set element's implicit anchor element to source.

        // Step 23. Run the popover focusing steps given element.
        self.run_popover_focusing_steps(cx);

        // Step 24. If shouldRestoreFocus is true and element's popover attribute is not in the No
        // Popover state, then set element's previously focused element to
        // originallyFocusedElement.
        if should_restore_focus && self.popover_state() != PopoverState::NoPopover {
            self.set_previously_focused_element(originally_focused_element.as_deref(), cx.no_gc());
        }

        // Step 25. Run cleanupShowingSteps.
        cleanup_showing_steps();

        // Step 26. Queue a popover toggle event task given element, "closed", "open", and source.
        self.queue_a_popover_toggle_event_task(
            cx.no_gc(),
            PopoverToggleState::Closed,
            PopoverToggleState::Open,
            source,
        );

        Ok(())
    }

    /// Implements the "If validityResult is not true" steps that are shared by
    /// [`Self::show_popover`] and [`Self::hide_popover`]. Returns `Ok(true)` if the algorithm
    /// should continue, `Ok(false)` if it should return, and an error if it should throw.
    fn handle_popover_validity(
        &self,
        validity_result: Fallible<bool>,
        throw_exceptions: bool,
    ) -> Fallible<bool> {
        match validity_result {
            Ok(validity) => Ok(validity),
            Err(error) if throw_exceptions => Err(error),
            Err(_) => Ok(false),
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#queue-a-popover-toggle-event-task>
    fn queue_a_popover_toggle_event_task(
        &self,
        no_gc: &NoGC,
        old_state: PopoverToggleState,
        new_state: PopoverToggleState,
        source: Option<&HTMLElement>,
    ) {
        let (old_state, generation) = self.with_popover_data_mut(no_gc, |data| {
            // Step 1. If element's popover toggle task tracker is not null, then:
            // Step 1.1. Set oldState to element's popover toggle task tracker's old state.
            // Step 1.2. Remove element's popover toggle task tracker's task from its task queue.
            // Step 1.3. Set element's popover toggle task tracker to null.
            let old_state = data.toggle_task_tracker.take().unwrap_or(old_state);
            data.toggle_task_generation = data.toggle_task_generation.wrapping_add(1);

            // Step 3. Set element's popover toggle task tracker to a struct with task set to the
            // just-queued task and old state set to oldState.
            data.toggle_task_tracker = Some(old_state);
            (old_state, data.toggle_task_generation)
        });

        // Step 2. Queue an element task given the DOM manipulation task source and element to run
        // the following steps:
        let this = Trusted::new(self);
        let source = source.map(Trusted::new);
        self.owner_global()
            .task_manager()
            .dom_manipulation_task_source()
            .queue(task!(fire_popover_toggle_event: move |cx| {
                let this = this.root();
                // This task was removed from its task queue by a later toggle event task.
                let superseded = this
                    .upcast::<Element>()
                    .popover_data()
                    .is_none_or(|data| data.toggle_task_generation != generation);
                if superseded {
                    return;
                }

                // Step 2.1. Fire an event named toggle at element, using ToggleEvent, with the
                // oldState attribute initialized to oldState, the newState attribute initialized
                // to newState, and the source attribute initialized to source.
                let source = source.map(|source| source.root());
                let event = ToggleEvent::new(
                    cx,
                    &this.owner_window(),
                    atom!("toggle"),
                    EventBubbles::DoesNotBubble,
                    EventCancelable::NotCancelable,
                    DOMString::from_static(old_state.as_str()),
                    DOMString::from_static(new_state.as_str()),
                    source.as_deref().map(|source| source.upcast::<Element>()),
                );
                event.upcast::<Event>().fire(cx, this.upcast::<EventTarget>());

                // Step 2.2. Set element's popover toggle task tracker to null.
                this.with_popover_data_mut(cx.no_gc(), |data| data.toggle_task_tracker = None);
            }));
    }

    /// <https://html.spec.whatwg.org/multipage/#hide-popover-algorithm>
    pub(crate) fn hide_popover(
        &self,
        cx: &mut JSContext,
        focus_previous_element: bool,
        mut fire_events: bool,
        throw_exceptions: bool,
        source: Option<&HTMLElement>,
    ) -> ErrorResult {
        // Step 1. Let validityResult be the result of running check popover validity given
        // element, true, and null. If this throws an exception, catch it, and let validityResult
        // be that exception.
        // Step 2. If validityResult is not true:
        // Step 2.1. If throwExceptions is true and validityResult is a DOMException, then throw
        // validityResult.
        // Step 2.2. Return.
        if !self
            .handle_popover_validity(self.check_popover_validity(true, None), throw_exceptions)?
        {
            return Ok(());
        }

        // Step 3. Let document be element's node document.
        let document = self.owner_document();
        let popovers = document.popover_data();

        // Step 4. Let nestedHide be element's popover hiding.
        // Step 5. Set element's popover hiding to true.
        let nested_hide = self
            .with_popover_data_mut(cx.no_gc(), |data| std::mem::replace(&mut data.hiding, true));

        // Step 6. If nestedHide is true, then set fireEvents to false.
        if nested_hide {
            fire_events = false;
        }

        // Step 7. Increment document's hiding popover nesting count.
        popovers
            .hiding_popover_nesting_count
            .set(popovers.hiding_popover_nesting_count.get() + 1);

        // Step 8. Let cleanupSteps be the following steps:
        let cleanup_steps = |no_gc: &NoGC| {
            // Step 8.1. If nestedHide is false, then set element's popover hiding to false.
            if !nested_hide {
                self.with_popover_data_mut(no_gc, |data| data.hiding = false);
            }
            // Step 8.2. Decrement document's hiding popover nesting count.
            popovers
                .hiding_popover_nesting_count
                .set(popovers.hiding_popover_nesting_count.get() - 1);
        };

        // Step 9. Let autoPopoverListContainsElement be true if document's showing auto popover
        // list contains element; otherwise false.
        let auto_popover_list_contains_element = document
            .showing_popover_list(PopoverMode::Auto)
            .iter()
            .any(|popover| &**popover == self);

        // Step 10. Let hintPopoverListContainsElement be true if document's showing hint popover
        // list contains element; otherwise false.
        let hint_popover_list_contains_element = document
            .showing_popover_list(PopoverMode::Hint)
            .iter()
            .any(|popover| &**popover == self);

        // Step 11. If element's opened in popover mode is "auto" or "hint":
        if self.opened_in_popover_mode().is_some() {
            // Step 11.1. If hintPopoverListContainsElement is true, then run hide popover stack
            // until given document, element, Hint, focusPreviousElement, and fireEvents.
            if hint_popover_list_contains_element {
                document.hide_popover_stack_until(
                    cx,
                    Some(self),
                    PopoverMode::Hint,
                    focus_previous_element,
                    fire_events,
                );
            }

            // Step 11.2. If element is document's hint stack parent, then run hide popover stack
            // until given document, null, Hint, focusPreviousElement, and fireEvents.
            if popovers
                .hint_stack_parent
                .get()
                .is_some_and(|hint_stack_parent| &*hint_stack_parent == self)
            {
                document.hide_popover_stack_until(
                    cx,
                    None,
                    PopoverMode::Hint,
                    focus_previous_element,
                    fire_events,
                );
            }

            // Step 11.3. If autoPopoverListContainsElement is true, then run hide popover stack
            // until given document, element, Auto, focusPreviousElement, and fireEvents.
            if auto_popover_list_contains_element {
                document.hide_popover_stack_until(
                    cx,
                    Some(self),
                    PopoverMode::Auto,
                    focus_previous_element,
                    fire_events,
                );
            }

            // Step 11.4. Set validityResult to the result of running check popover validity given
            // element, true, and document. If this throws an exception, catch it, and set
            // validityResult to that exception.
            // Step 11.5. If validityResult is not true:
            // Step 11.5.1. Run cleanupSteps.
            // Step 11.5.2. If throwExceptions is true and validityResult is a DOMException, then
            // throw validityResult.
            // Step 11.5.3. Return.
            let validity = self.check_popover_validity(true, Some(&document));
            if !self
                .handle_popover_validity(validity, throw_exceptions)
                .inspect_err(|_| cleanup_steps(cx.no_gc()))?
            {
                cleanup_steps(cx.no_gc());
                return Ok(());
            }
        }

        // Step 12. If fireEvents is true:
        if fire_events {
            // Step 12.1. Fire an event named beforetoggle, using ToggleEvent, with the oldState
            // attribute initialized to "open", the newState attribute initialized to "closed", and
            // the source attribute set to source at element.
            let event = ToggleEvent::new(
                cx,
                &self.owner_window(),
                atom!("beforetoggle"),
                EventBubbles::DoesNotBubble,
                EventCancelable::NotCancelable,
                DOMString::from_static("open"),
                DOMString::from_static("closed"),
                source.map(|source| source.upcast::<Element>()),
            );
            event
                .upcast::<Event>()
                .fire(cx, self.upcast::<EventTarget>());

            // Step 12.2. Set validityResult to the result of running check popover validity given
            // element, true, and document. If this throws an exception, catch it, and set
            // validityResult to that exception.
            // Step 12.3. If validityResult is not true:
            // Step 12.3.1. Run cleanupSteps.
            // Step 12.3.2. If throwExceptions is true and validityResult is a DOMException, then
            // throw validityResult.
            // Step 12.3.3. Return.
            let validity = self.check_popover_validity(true, Some(&document));
            if !self
                .handle_popover_validity(validity, throw_exceptions)
                .inspect_err(|_| cleanup_steps(cx.no_gc()))?
            {
                cleanup_steps(cx.no_gc());
                return Ok(());
            }

            // Step 12.4. Request an element to be removed from the top layer given element.
            document.request_an_element_to_be_removed_from_the_top_layer(
                cx.no_gc(),
                self.upcast::<Element>(),
            );

            // TODO: Step 12.5. Set element's implicit anchor element to null.
        }
        // Step 13. Otherwise, remove an element from the top layer immediately given element.
        else {
            document.remove_an_element_from_the_top_layer_immediately(
                cx.no_gc(),
                self.upcast::<Element>(),
            );
        }

        // Step 14. Set element's popover trigger to null.
        // Step 15. Set element's opened in popover mode to null.
        self.with_popover_data_mut(cx.no_gc(), |data| {
            data.trigger = None;
            data.opened_in_popover_mode = None;
        });

        // Step 16. Set element's popover visibility state to hidden.
        self.upcast::<Element>()
            .set_state(cx.no_gc(), ElementState::POPOVER_OPEN, false);

        // Step 17. If element is document's hint stack parent, or document's showing hint popover
        // list is empty, then set document's hint stack parent to null.
        if popovers
            .hint_stack_parent
            .get()
            .is_some_and(|hint_stack_parent| &*hint_stack_parent == self) ||
            document.showing_popover_list(PopoverMode::Hint).is_empty()
        {
            popovers.hint_stack_parent.set(None);
        }

        // Step 18. If fireEvents is true, then queue a popover toggle event task given element,
        // "open", "closed", and source.
        if fire_events {
            self.queue_a_popover_toggle_event_task(
                cx.no_gc(),
                PopoverToggleState::Open,
                PopoverToggleState::Closed,
                source,
            );
        }

        // Step 19. Let previouslyFocusedElement be element's previously focused element.
        // Step 20. If previouslyFocusedElement is not null:
        if let Some(previously_focused_element) = self.previously_focused_element(cx.no_gc()) {
            // Step 20.1. Set element's previously focused element to null.
            self.set_previously_focused_element(None, cx.no_gc());

            // Step 20.2. If focusPreviousElement is true and document's focused area of the
            // document's DOM anchor is a shadow-including inclusive descendant of element, then
            // run the focusing steps for previouslyFocusedElement; the viewport should not be
            // scrolled by doing this step.
            if focus_previous_element {
                let focused_node = document
                    .focus_handler()
                    .focused_area()
                    .dom_anchor(&document);
                if focused_node
                    .inclusive_ancestors(ShadowIncluding::Yes)
                    .any(|ancestor| &*ancestor == self.upcast::<Node>())
                {
                    previously_focused_element
                        .upcast::<Node>()
                        .run_the_focusing_steps(cx, None, FocusTrigger::Script);
                }
            }
        }

        // Step 21. If element's popover close watcher is not null:
        // Step 21.1. Destroy element's popover close watcher.
        // Step 21.2. Set element's popover close watcher to null.
        let close_watcher =
            self.with_popover_data_mut(cx.no_gc(), |data| data.close_watcher.take());
        if let Some(close_watcher) = close_watcher {
            close_watcher.destroy(cx.no_gc());
        }

        // Step 22. Run cleanupSteps.
        cleanup_steps(cx.no_gc());
        Ok(())
    }

    /// <https://html.spec.whatwg.org/multipage/#popover-focusing-steps>
    fn run_popover_focusing_steps(&self, cx: &mut JSContext) {
        // TODO: Step 1. If the allow focus steps given subject's node document return false, then
        // return.

        // Step 2. If subject is a dialog element, then run the dialog focusing steps given subject
        // and return.
        if let Some(dialog) = self.downcast::<HTMLDialogElement>() {
            dialog.run_dialog_focusing_steps(cx);
            return;
        }

        // Step 3. If subject has the autofocus attribute, then let control be subject.
        // Step 6. Run the focusing steps given control.
        if self
            .upcast::<Element>()
            .has_attribute(&local_name!("autofocus"))
        {
            self.upcast::<Node>()
                .run_the_focusing_steps(cx, None, FocusTrigger::Script);
            return;
        }

        // Step 4. Otherwise, let control be the autofocus delegate for subject given "other".
        rooted!(&in(cx) let control = self.upcast::<Node>().autofocus_delegate(cx, FocusTrigger::Script));

        // Step 5. If control is null, then return.
        let Some(control) = &*control else {
            return;
        };

        // Step 6. Run the focusing steps given control.
        self.owner_document()
            .focus_handler()
            .focus(cx, control, FocusTrigger::Script);

        // TODO: Step 7. Let topDocument be control's node navigable's top-level traversable's
        // active document.
        // TODO: Step 8. If control's node document's origin is not the same as the origin of
        // topDocument, then return.
        // TODO: Step 9. Empty topDocument's autofocus candidates.
        // TODO: Step 10. Set topDocument's autofocus processed flag to true.
    }
}

impl Document {
    /// <https://html.spec.whatwg.org/multipage/#showing-auto-popover-list>
    /// <https://html.spec.whatwg.org/multipage/#showing-hint-popover-list>
    pub(crate) fn showing_popover_list(&self, mode: PopoverMode) -> Vec<DomRoot<HTMLElement>> {
        // Step 1. Let popovers be « ».
        // Step 2. For each Element element of document's top layer:
        // Step 2.1. If all of the following are true:
        // - element is an HTML element;
        // - element's opened in popover mode is mode; and
        // - element's popover visibility state is showing,
        // then append element to popovers.
        // Step 3. Return popovers.
        self.top_layer_elements()
            .into_iter()
            .filter_map(DomRoot::downcast::<HTMLElement>)
            .filter(|element| {
                element.opened_in_popover_mode() == Some(mode) && element.is_popover_showing()
            })
            .collect()
    }

    /// <https://html.spec.whatwg.org/multipage/#topmost-auto-popover>
    pub(crate) fn topmost_auto_or_hint_popover(&self) -> Option<DomRoot<HTMLElement>> {
        // Step 1. If document's showing hint popover list is not empty, then return document's
        // showing hint popover list's last element.
        // Step 2. If document's showing auto popover list is not empty, then return document's
        // showing auto popover list's last element.
        // Step 3. Return null.
        self.showing_popover_list(PopoverMode::Hint)
            .pop()
            .or_else(|| self.showing_popover_list(PopoverMode::Auto).pop())
    }

    /// <https://html.spec.whatwg.org/multipage/#hide-popover-stack-until>
    pub(crate) fn hide_popover_stack_until(
        &self,
        cx: &mut JSContext,
        endpoint: Option<&HTMLElement>,
        stack_type: PopoverMode,
        focus_previous_element: bool,
        fire_events: bool,
    ) {
        // Step 1. Let popoverList be document's showing auto popover list if stackType is Auto;
        // otherwise document's showing hint popover list.
        let popover_list = self.showing_popover_list(stack_type);

        // Step 2. Let lastHideIndex be 0 if popoverList does not contain endpoint; otherwise the
        // index of endpoint in popoverList plus 1.
        let last_hide_index = endpoint
            .and_then(|endpoint| {
                popover_list
                    .iter()
                    .position(|popover| &**popover == endpoint)
            })
            .map_or(0, |index| index + 1);

        // Step 3. Let toHide be a slice of popoverList from lastHideIndex, in reverse order.
        // Step 4. Let toRemain be a slice of popoverList from 0 to lastHideIndex.
        let (to_remain, to_hide) = popover_list.split_at(last_hide_index);

        // Step 5. For each popover of toHide: run the hide popover algorithm given popover,
        // focusPreviousElement, fireEvents, false, and null.
        for popover in to_hide.iter().rev() {
            let _ = popover.hide_popover(cx, focus_previous_element, fire_events, false, None);
        }

        // Step 6. Let newPopoverList be document's showing auto popover list if stackType is
        // Auto; otherwise document's showing hint popover list.
        // Step 7. Let toCheck be newPopoverList in reverse order.
        // Step 8. For each popover of toCheck:
        for popover in self.showing_popover_list(stack_type).iter().rev() {
            // Step 8.1. If toRemain contains popover, then continue.
            if to_remain.contains(popover) {
                continue;
            }
            // Step 8.2. Run the hide popover algorithm given popover, focusPreviousElement,
            // false, false, and null.
            let _ = popover.hide_popover(cx, focus_previous_element, false, false, None);
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#hide-all-popovers-until>
    ///
    /// A `None` endpoint stands for the document.
    pub(crate) fn hide_popovers_until(
        &self,
        cx: &mut JSContext,
        endpoint: Option<&HTMLElement>,
        focus_previous_element: bool,
        fire_events: bool,
    ) {
        // Step 1. Let endpointIsHint be true if document's showing hint popover list contains
        // endpoint; otherwise false.
        let endpoint_is_hint = endpoint.is_some_and(|endpoint| {
            self.showing_popover_list(PopoverMode::Hint)
                .iter()
                .any(|popover| &**popover == endpoint)
        });

        // Step 2. Run hide popover stack until given document, endpoint, Hint,
        // focusPreviousElement, and fireEvents.
        self.hide_popover_stack_until(
            cx,
            endpoint,
            PopoverMode::Hint,
            focus_previous_element,
            fire_events,
        );

        // Step 3. Let autoEndpoint be endpoint.
        // Step 4. If endpointIsHint is true, then set autoEndpoint to document's hint stack
        // parent.
        let auto_endpoint = if endpoint_is_hint {
            self.popover_data().hint_stack_parent.get()
        } else {
            endpoint.map(DomRoot::from_ref)
        };

        // Step 5. Run hide popover stack until given document, autoEndpoint, Auto,
        // focusPreviousElement, and fireEvents.
        self.hide_popover_stack_until(
            cx,
            auto_endpoint.as_deref(),
            PopoverMode::Auto,
            focus_previous_element,
            fire_events,
        );
    }
}

/// <https://html.spec.whatwg.org/multipage/#topmost-popover-ancestor>
pub(crate) fn topmost_popover_ancestor(
    no_gc: &NoGC,
    new_popover_or_top_layer_element: &Node,
    source: Option<&HTMLElement>,
    is_popover: bool,
) -> Option<DomRoot<HTMLElement>> {
    // Step 1. If isPopover is true:
    // Step 1.1. Assert: newPopoverOrTopLayerElement is an HTML element.
    // Step 1.2. Assert: newPopoverOrTopLayerElement's popover attribute is not in the No Popover
    // State or the Manual state.
    // Step 1.3. Assert: newPopoverOrTopLayerElement's popover visibility state is not in the
    // popover showing state.
    // Step 2. Otherwise:
    // Step 2.1. Assert: source is null.
    debug_assert!(is_popover || source.is_none());

    // Step 3. Let document be newPopoverOrTopLayerElement's node document.
    let document = new_popover_or_top_layer_element.owner_doc();

    // Step 4. Let combinedPopovers be document's showing auto popover list extended with
    // document's showing hint popover list.
    let mut combined_popovers = document.showing_popover_list(PopoverMode::Auto);
    combined_popovers.extend(document.showing_popover_list(PopoverMode::Hint));

    // Returns the index of the last item in combinedPopovers of which node is a flat tree
    // descendant, if any.
    let last_flat_tree_ancestor_index = |node: &Node| {
        combined_popovers.iter().rposition(|popover| {
            node.ancestors_in_flat_tree_unrooted(no_gc)
                .any(|ancestor| &**ancestor == popover.upcast::<Node>())
        })
    };

    // Step 5. Let popoverAncestorIndex be the index of the last item in combinedPopovers of which
    // newPopoverOrTopLayerElement is a flat tree descendant, otherwise -1.
    let popover_ancestor_index = last_flat_tree_ancestor_index(new_popover_or_top_layer_element);

    // Step 6. Let sourceAncestorIndex be -1.
    // Step 7. If source is not null, then set sourceAncestorIndex to the index of the last item
    // in combinedPopovers of which source is a flat tree descendant, otherwise -1.
    let source_ancestor_index =
        source.and_then(|source| last_flat_tree_ancestor_index(source.upcast()));

    // Step 8. Let ancestorIndex be the maximum of popoverAncestorIndex and sourceAncestorIndex.
    // Step 9. If ancestorIndex is -1, then return null.
    let ancestor_index = popover_ancestor_index.max(source_ancestor_index)?;

    // Step 10. Return combinedPopovers[ancestorIndex].
    Some(combined_popovers.swap_remove(ancestor_index))
}

/// The states of the `popovertargetaction` attribute.
///
/// <https://html.spec.whatwg.org/multipage/#attr-popovertargetaction>
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PopoverTargetAction {
    Toggle,
    Show,
    Hide,
}

impl PopoverTargetAction {
    fn from_attribute(element: &Element) -> PopoverTargetAction {
        // > The attribute's missing value default and invalid value default are both the Toggle
        // > state.
        match element.get_attribute_string_ref(&local_name!("popovertargetaction")) {
            Some(value) if value.eq_ignore_ascii_case("show") => PopoverTargetAction::Show,
            Some(value) if value.eq_ignore_ascii_case("hide") => PopoverTargetAction::Hide,
            _ => PopoverTargetAction::Toggle,
        }
    }
}

/// Returns the first element in tree order whose root is the root of `element` and whose ID is
/// `id`.
fn element_with_id_in_same_tree(
    no_gc: &NoGC,
    element: &Element,
    id: &str,
) -> Option<DomRoot<Element>> {
    if id.is_empty() {
        return None;
    }
    let id = Atom::from(id);
    let root = element
        .upcast::<Node>()
        .GetRootNode(&GetRootNodeOptions::empty());
    if let Some(document) = root.downcast::<Document>() {
        return document.get_element_by_id(no_gc, &id);
    }
    if let Some(shadow_root) = root.downcast::<ShadowRoot>() {
        return shadow_root
            .upcast::<DocumentFragment>()
            .id_map()
            .get(no_gc, &root, &id);
    }
    root.traverse_preorder(ShadowIncluding::No)
        .filter_map(DomRoot::downcast::<Element>)
        .find(|candidate| candidate.get_id().as_ref() == Some(&id))
}

/// The getter steps of `popoverTargetElement`, which reflects the `popovertarget` attribute.
///
/// <https://html.spec.whatwg.org/multipage/#attr-associated-element>
pub(crate) fn get_the_popovertarget_associated_element(
    no_gc: &NoGC,
    element: &Element,
    explicitly_set_attr_element: Option<DomRoot<Element>>,
) -> Option<DomRoot<Element>> {
    // Step 1. Let element be the result of running reflectedTarget's get the element.
    // Step 2. Let contentAttributeValue be the result of running reflectedTarget's get the
    // content attribute.
    // Step 3. If reflectedTarget's explicitly set attr-element is not null:
    if let Some(explicitly_set_attr_element) = explicitly_set_attr_element {
        // Step 3.1. If reflectedTarget's explicitly set attr-element is a descendant of any of
        // element's shadow-including ancestors, then return reflectedTarget's explicitly set
        // attr-element.
        // Step 3.2. Return null.
        let is_in_scope = element
            .upcast::<Node>()
            .inclusive_ancestors(ShadowIncluding::Yes)
            .skip(1)
            .any(|ancestor| ancestor.is_ancestor_of(explicitly_set_attr_element.upcast()));
        return is_in_scope.then_some(explicitly_set_attr_element);
    }

    // Step 4. Otherwise, if contentAttributeValue is not null, return the first element candidate,
    // in tree order, that meets the following criteria:
    // - candidate's root is the same as element's root;
    // - candidate's ID is contentAttributeValue; and
    // - candidate implements T.
    // If no such element exists, then return null.
    // Step 5. Return null.
    let value = element.get_attribute_string_value(&local_name!("popovertarget"))?;
    element_with_id_in_same_tree(no_gc, element, &value)
}

/// The setter steps of `popoverTargetElement`, which reflects the `popovertarget` attribute.
///
/// <https://html.spec.whatwg.org/multipage/#reflecting-content-attributes-in-idl-attributes:element>
pub(crate) fn set_popover_target_element(
    cx: &mut JSContext,
    element: &Element,
    explicitly_set_attr_element: &MutNullableDom<Element>,
    value: Option<&Element>,
) {
    // Step 1. If the given value is null:
    let Some(value) = value else {
        // Step 1.1. Set this's explicitly set attr-element to null.
        explicitly_set_attr_element.set(None);
        // Step 1.2. Run this's delete the content attribute.
        element.remove_attribute(cx, &ns!(), &local_name!("popovertarget"));
        // Step 1.3. Return.
        return;
    };

    // Step 2. Run this's set the content attribute with the empty string.
    element.set_string_attribute(cx, &local_name!("popovertarget"), DOMString::new());

    // Step 3. Set this's explicitly set attr-element to a weak reference to the given value.
    explicitly_set_attr_element.set(Some(value));
}

/// Returns whether `node` is a button, and if so whether it is a submit button.
///
/// <https://html.spec.whatwg.org/multipage/#concept-button>
/// <https://html.spec.whatwg.org/multipage/#concept-submit-button>
fn button_kind(node: &Node) -> Option<(&HTMLElement, bool)> {
    if let Some(button) = node.downcast::<HTMLButtonElement>() {
        return Some((button.upcast(), button.is_submit_button()));
    }
    let input = node.downcast::<HTMLInputElement>()?;
    let is_submit_button = match *input.input_type() {
        InputType::Submit(_) | InputType::Image(_) => true,
        InputType::Reset(_) | InputType::Button(_) => false,
        _ => return None,
    };
    Some((input.upcast(), is_submit_button))
}

/// <https://html.spec.whatwg.org/multipage/#popover-target-element>
pub(crate) fn popover_target_element(no_gc: &NoGC, node: &Node) -> Option<DomRoot<HTMLElement>> {
    // Step 1. If node is not a button, then return null.
    let (button, is_submit_button) = button_kind(node)?;

    // Step 2. If node is disabled, then return null.
    let element = button.upcast::<Element>();
    if element.disabled_state() {
        return None;
    }

    // Step 3. If node has a form owner and node is a submit button, then return null.
    if is_submit_button && button.form_owner().is_some() {
        return None;
    }

    // Step 4. Let popoverElement be the result of running node's get the
    // popovertarget-associated element.
    let explicitly_set_attr_element = if let Some(button) = node.downcast::<HTMLButtonElement>() {
        button.explicitly_set_popover_target_element()
    } else {
        node.downcast::<HTMLInputElement>()
            .and_then(HTMLInputElement::explicitly_set_popover_target_element)
    };
    let popover_element =
        get_the_popovertarget_associated_element(no_gc, element, explicitly_set_attr_element)?;

    // Step 5. If popoverElement is null, then return null.
    // Step 6. If popoverElement's popover attribute is in the No Popover state, then return null.
    // Step 7. Return popoverElement.
    DomRoot::downcast::<HTMLElement>(popover_element)
        .filter(|popover| popover.popover_state() != PopoverState::NoPopover)
}

/// <https://html.spec.whatwg.org/multipage/#popover-target-attribute-activation-behavior>
pub(crate) fn run_the_popover_target_attribute_activation_behavior(
    cx: &mut JSContext,
    node: &HTMLElement,
    event_target: &EventTarget,
) {
    // Step 1. Let popover be node's popover target element.
    // Step 2. If popover is null, then return.
    let Some(popover) = popover_target_element(cx.no_gc(), node.upcast()) else {
        return;
    };

    // Step 3. If eventTarget is a shadow-including inclusive descendant of popover and popover is
    // a shadow-including descendant of node, then return.
    if event_target.downcast::<Node>().is_some_and(|event_target| {
        popover
            .upcast::<Node>()
            .is_shadow_including_inclusive_ancestor_of(event_target)
    }) && popover.upcast::<Node>() != node.upcast::<Node>() &&
        node.upcast::<Node>()
            .is_shadow_including_inclusive_ancestor_of(popover.upcast())
    {
        return;
    }

    // Step 4. If node's popovertargetaction attribute is in the show state and popover's popover
    // visibility state is showing, then return.
    // Step 5. If node's popovertargetaction attribute is in the hide state and popover's popover
    // visibility state is hidden, then return.
    let action = PopoverTargetAction::from_attribute(node.upcast());
    if (action == PopoverTargetAction::Show && popover.is_popover_showing()) ||
        (action == PopoverTargetAction::Hide && !popover.is_popover_showing())
    {
        return;
    }

    // Step 6. If popover's popover visibility state is showing, then run the hide popover
    // algorithm given popover, true, true, false, and node.
    if popover.is_popover_showing() {
        let _ = popover.hide_popover(cx, true, true, false, Some(node));
    }
    // Step 7. Otherwise:
    else {
        // Step 7.1. Let validity be the result of running check popover validity given popover,
        // false, and null. If this throws an exception, catch it, and let validity be false.
        // Step 7.2. If validity is true, then run show popover given popover, false, and node.
        if matches!(popover.check_popover_validity(false, None), Ok(true)) {
            let _ = popover.show_popover(cx, false, Some(node));
        }
    }
}

/// <https://html.spec.whatwg.org/multipage/#popover-light-dismiss>
///
/// This is called with a trusted `pointerdown` or `pointerup` event and the target it is about
/// to be dispatched to, before dispatching it.
pub(crate) fn light_dismiss_open_popovers(cx: &mut JSContext, event: &Event, target: &EventTarget) {
    // Step 1. Assert: event's isTrusted attribute is true.
    // Step 2. Let target be event's target.
    let Some(target) = target.downcast::<Node>() else {
        return;
    };

    // Step 3. Let document be target's node document.
    let document = target.owner_doc();

    // Step 4. If the result of running topmost auto or hint popover given document is null, then
    // return.
    if document.topmost_auto_or_hint_popover().is_none() {
        return;
    }

    let popovers = document.popover_data();
    let event_type = event.type_();
    // Step 5. If event's type is "pointerdown": set document's popover pointerdown target to the
    // result of running topmost clicked popover given target.
    if &*event_type == "pointerdown" {
        let topmost_clicked_popover = topmost_clicked_popover(cx.no_gc(), target);
        popovers
            .pointerdown_target
            .set(topmost_clicked_popover.as_deref());
    }
    // Step 6. If event's type is "pointerup":
    else if &*event_type == "pointerup" {
        // Step 6.1. Let ancestor be the result of running topmost clicked popover given target.
        let ancestor = topmost_clicked_popover(cx.no_gc(), target);

        // Step 6.2. Let sameTarget be true if ancestor is document's popover pointerdown target.
        let same_target = ancestor == popovers.pointerdown_target.get();

        // Step 6.3. Set document's popover pointerdown target to null.
        popovers.pointerdown_target.set(None);

        // Step 6.4. If sameTarget is false, then return.
        if !same_target {
            return;
        }

        // Step 6.5. Run hide popovers until given document, ancestor, false, and true.
        document.hide_popovers_until(cx, ancestor.as_deref(), false, true);
    }
}

/// <https://html.spec.whatwg.org/multipage/#topmost-clicked-popover>
fn topmost_clicked_popover(no_gc: &NoGC, node: &Node) -> Option<DomRoot<HTMLElement>> {
    // Step 1. Let clickedPopover be the result of running nearest inclusive open popover given
    // node.
    let clicked_popover = nearest_inclusive_open_popover(no_gc, node);

    // Step 2. Let targetPopover be the result of running nearest inclusive target popover given
    // node.
    let target_popover = nearest_inclusive_target_popover(no_gc, node);

    // Step 3. If the result of getting the popover stack position given clickedPopover is greater
    // than the result of getting the popover stack position given targetPopover, then return
    // clickedPopover.
    if popover_stack_position(clicked_popover.as_deref()) >
        popover_stack_position(target_popover.as_deref())
    {
        return clicked_popover;
    }

    // Step 4. Return targetPopover.
    target_popover
}

/// <https://html.spec.whatwg.org/multipage/#popover-stack-position>
fn popover_stack_position(popover: Option<&HTMLElement>) -> usize {
    let Some(popover) = popover else {
        return 0;
    };
    let document = popover.owner_document();

    // Step 1. Let hintList be popover's node document's showing hint popover list.
    let hint_list = document.showing_popover_list(PopoverMode::Hint);

    // Step 2. Let autoList be popover's node document's showing auto popover list.
    let auto_list = document.showing_popover_list(PopoverMode::Auto);

    // Step 3. If popover is in hintList, then return the index of popover in hintList + the size
    // of autoList + 1.
    if let Some(index) = hint_list.iter().position(|element| &**element == popover) {
        return index + auto_list.len() + 1;
    }

    // Step 4. If popover is in autoList, then return the index of popover in autoList + 1.
    if let Some(index) = auto_list.iter().position(|element| &**element == popover) {
        return index + 1;
    }

    // Step 5. Return 0.
    0
}

/// <https://html.spec.whatwg.org/multipage/#nearest-inclusive-open-popover>
fn nearest_inclusive_open_popover(no_gc: &NoGC, node: &Node) -> Option<DomRoot<HTMLElement>> {
    // Step 1. Let currentNode be node.
    // Step 2. While currentNode is not null:
    // Step 2.1. If currentNode's opened in popover mode is "auto" or "hint", and currentNode's
    // popover visibility state is showing, then return currentNode.
    // Step 2.2. Set currentNode to currentNode's parent in the flat tree.
    // Step 3. Return null.
    node.inclusive_ancestors_in_flat_tree_unrooted(no_gc)
        .filter_map(|ancestor| ancestor.downcast::<HTMLElement>().map(DomRoot::from_ref))
        .find(|element| element.is_popover_showing() && element.opened_in_popover_mode().is_some())
}

/// <https://html.spec.whatwg.org/multipage/#nearest-inclusive-target-popover>
fn nearest_inclusive_target_popover(no_gc: &NoGC, node: &Node) -> Option<DomRoot<HTMLElement>> {
    // Step 1. Let currentNode be node.
    // Step 2. While currentNode is not null:
    // Step 2.1. Let targetPopover be currentNode's target popover.
    // Step 2.2. If targetPopover is not null and targetPopover's popover attribute is in the Auto
    // state or the Hint state, and targetPopover's popover visibility state is showing, then
    // return targetPopover.
    // Step 2.3. Set currentNode to currentNode's ancestor in the flat tree.
    node.inclusive_ancestors_in_flat_tree_unrooted(no_gc)
        .filter_map(|ancestor| target_popover(no_gc, &ancestor))
        .find(|target_popover| {
            matches!(
                target_popover.popover_state(),
                PopoverState::Auto | PopoverState::Hint
            ) && target_popover.is_popover_showing()
        })
}

/// <https://html.spec.whatwg.org/multipage/#target-popover>
fn target_popover(no_gc: &NoGC, node: &Node) -> Option<DomRoot<HTMLElement>> {
    // Step 1. If node is not a button element, then return node's popover target element.
    let Some(button) = node.downcast::<HTMLButtonElement>() else {
        return popover_target_element(no_gc, node);
    };

    // Step 2. If node is disabled, then return null.
    let element = button.upcast::<Element>();
    if element.disabled_state() {
        return None;
    }

    // Step 3. Let target be the result of running node's get the commandfor-associated element.
    // Step 4. If target is null, then return node's popover target element.
    let Some(target) = element
        .get_attribute_string_value(&local_name!("commandfor"))
        .and_then(|value| element_with_id_in_same_tree(no_gc, element, &value))
    else {
        return popover_target_element(no_gc, node);
    };

    // Step 5. If node has a form owner:
    // Step 5.1. If element is a submit button, then return null.
    // Step 5.2. If element's type attribute is in the Reset Button state, then return null.
    // Step 5.3. If element's type attribute is in the Auto state, then return null.
    if button.form_owner().is_some() && !button.type_attribute_is_in_the_button_state() {
        return None;
    }

    // Step 6. Let command be node's command attribute.
    // Step 7. If command is not in the Toggle Popover, Hide Popover, or Show Popover states, then
    // return null.
    if !button.has_popover_command() {
        return None;
    }

    // Step 8. If target's popover attribute is in the No Popover state, then return null.
    // Step 9. Return target.
    DomRoot::downcast::<HTMLElement>(target)
        .filter(|target| target.popover_state() != PopoverState::NoPopover)
}
