/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use std::cell::Cell;
use std::rc::Rc;

use dom_struct::dom_struct;
use euclid::Point2D;
use html5ever::{LocalName, Prefix, local_name, ns};
use js::context::{JSContext, NoGC};
use js::rust::HandleObject;
use script_bindings::cell::DomRefCell;
use script_bindings::codegen::GenericBindings::HTMLElementBinding::HTMLElementMethods;
use script_bindings::domstring::TracedDOMString;
use script_bindings::error::{Error, ErrorResult};
use script_bindings::root::Dom;
use style_traits::CSSPixel;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::HTMLDialogElementBinding::HTMLDialogElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::refcounted::Trusted;
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::closewatcher::internalclosewatcher::{
    InternalCloseWatcher, InternalCloseWatcherOwner,
};
use crate::dom::document::Document;
use crate::dom::document::focus::FocusableArea;
use crate::dom::element::storage::AttrRef;
use crate::dom::element::{AttributeMutation, Element};
use crate::dom::event::{Event, EventBubbles, EventCancelable};
use crate::dom::eventtarget::EventTarget;
use crate::dom::html::htmlelement::HTMLElement;
use crate::dom::html::popover::topmost_popover_ancestor;
use crate::dom::htmlbuttonelement::{CommandState, HTMLButtonElement};
use crate::dom::node::focus::FocusTrigger;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::node::{BindContext, Node, NodeTraits, UnbindContext};
use crate::dom::toggleevent::ToggleEvent;

#[dom_struct]
pub(crate) struct HTMLDialogElement {
    htmlelement: HTMLElement,
    return_value: DomRefCell<TracedDOMString>,
    /// <https://html.spec.whatwg.org/multipage/#dialog-close-watcher>
    #[conditional_malloc_size_of]
    close_watcher: DomRefCell<Option<Rc<InternalCloseWatcher>>>,
    /// <https://html.spec.whatwg.org/multipage/#request-close-return-value>
    request_close_return_value: DomRefCell<Option<TracedDOMString>>,
    /// <https://html.spec.whatwg.org/multipage/#request-close-source-element>
    request_close_source_element: MutNullableDom<Element>,
    /// <https://html.spec.whatwg.org/multipage/#enable-close-watcher-for-requestclose()>
    enable_close_watcher_for_request_close: Cell<bool>,
    /// <https://html.spec.whatwg.org/multipage/#dialog-toggle-task-tracker>
    ///
    /// The old state of the pending toggle event, and a counter that tells the pending task apart
    /// from the ones that it replaced.
    toggle_task_old_state: Cell<Option<&'static str>>,
    toggle_task_counter: Cell<u32>,
}

impl HTMLDialogElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> HTMLDialogElement {
        HTMLDialogElement {
            htmlelement: HTMLElement::new_inherited(local_name, prefix, document),
            return_value: DomRefCell::new(DOMString::new().traced()),
            close_watcher: Default::default(),
            request_close_return_value: Default::default(),
            request_close_source_element: Default::default(),
            toggle_task_old_state: Cell::new(None),
            toggle_task_counter: Cell::new(0),
            enable_close_watcher_for_request_close: Default::default(),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<HTMLDialogElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(HTMLDialogElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }

    /// <https://html.spec.whatwg.org/multipage/#show-a-modal-dialog>
    pub(crate) fn show_a_modal(
        &self,
        cx: &mut js::context::JSContext,
        source: Option<DomRoot<Element>>,
    ) -> ErrorResult {
        let subject = self.upcast::<Element>();
        // Step 1. If subject has an open attribute and is modal of subject is true, then return.
        if subject.has_attribute(&local_name!("open")) &&
            subject.state().contains(ElementState::MODAL)
        {
            return Ok(());
        }

        // Step 2. If subject has an open attribute, then throw an "InvalidStateError" DOMException.
        if subject.has_attribute(&local_name!("open")) {
            return Err(Error::InvalidState(Some(
                "Cannot call showModal() on an already open dialog.".into(),
            )));
        }

        // Step 3. If subject's node document is not fully active, then throw an "InvalidStateError" DOMException.
        if !subject.owner_document().is_fully_active() {
            return Err(Error::InvalidState(Some(
                "Cannot call showModal() on a dialog whose document is not fully active.".into(),
            )));
        }

        // Step 4. If subject is not connected, then throw an "InvalidStateError" DOMException.
        if !subject.is_connected() {
            return Err(Error::InvalidState(Some(
                "Cannot call showModal() on a dialog that is not connected.".into(),
            )));
        }

        // Step 5. If subject is in the popover showing state, then throw an "InvalidStateError" DOMException.
        if self.upcast::<HTMLElement>().is_popover_showing() {
            return Err(Error::InvalidState(Some(
                "Cannot call showModal() on a dialog that is showing as a popover.".into(),
            )));
        }

        // Step 6. If the result of firing an event named beforetoggle, using ToggleEvent, with the cancelable attribute initialized to true, the oldState attribute initialized to "closed", the newState attribute initialized to "open", and the source attribute initialized to source at subject is false, then return.
        let event = ToggleEvent::new(
            cx,
            &self.owner_window(),
            atom!("beforetoggle"),
            EventBubbles::DoesNotBubble,
            EventCancelable::Cancelable,
            DOMString::from_static("closed"),
            DOMString::from_static("open"),
            source.as_deref(),
        );
        let event = event.upcast::<Event>();
        if !event.fire(cx, self.upcast::<EventTarget>()) {
            return Ok(());
        }

        // Step 7. If subject has an open attribute, then return.
        if subject.has_attribute(&local_name!("open")) {
            return Ok(());
        }

        // Step 8. If subject is not connected, then return.
        if !subject.is_connected() {
            return Ok(());
        }

        // Step 9. If subject is in the popover showing state, then return.
        if self.upcast::<HTMLElement>().is_popover_showing() {
            return Ok(());
        }

        // Step 10. Queue a dialog toggle event task given subject, "closed", "open", and source.
        self.queue_dialog_toggle_event_task("closed", "open", source);

        // Step 11. Add an open attribute to subject, whose value is the empty string.
        subject.set_bool_attribute(cx, &local_name!("open"), true);
        subject.set_open_state(cx.no_gc(), true);

        // TODO: Step 12. Assert: subject's close watcher is not null.

        // Step 13. Set is modal of subject to true.
        self.upcast::<Element>().set_modal_state(cx.no_gc(), true);

        // Step 14. Set subject's node document to be blocked by the modal dialog subject.
        // Step 15. If subject's node document's top layer does not already contain subject, then
        // add an element to the top layer given subject.
        //
        // Adding a modal element to the top layer is what makes the rest of the document inert.
        let document = self.owner_document();
        if !document.top_layer_contains(subject) {
            document.add_an_element_to_the_top_layer(cx.no_gc(), subject);
        }

        // Step 16. Set subject's previously focused element to the focused element.
        self.upcast::<HTMLElement>().set_previously_focused_element(
            self.owner_document()
                .focus_handler()
                .focused_area()
                .element(),
            cx.no_gc(),
        );

        // Step 17. Let document be subject's node document.
        // Step 18. Let hideUntil be the result of running topmost popover ancestor given subject,
        // null, and false.
        // Step 19. Run hide popovers until given document, hideUntil, false, and true.
        self.hide_popovers_until_topmost_popover_ancestor(cx);

        // Step 20. Run the dialog focusing steps given subject.
        self.run_dialog_focusing_steps(cx);
        Ok(())
    }

    /// <https://html.spec.whatwg.org/multipage/#set-the-dialog-close-watcher>
    pub(crate) fn set_the_close_watcher(&self, no_gc: &NoGC) {
        // Step 1. Assert: dialog's close watcher is null.
        assert!(self.close_watcher.borrow().is_none());

        // Step 2. Assert: dialog has an open attribute and dialog's node document is fully active.
        let dialog = self.upcast::<Element>();
        assert!(
            dialog.has_attribute(&local_name!("open")) && dialog.owner_document().is_fully_active()
        );

        // Step 3. Set dialog's close watcher to the result of establishing a close watcher given
        // dialog's relevant global object, with:
        // - cancelAction given canPreventClose being to return the result of firing an event named
        //   cancel at dialog, with the cancelable attribute initialized to canPreventClose.
        // - closeAction being to close the dialog given dialog, dialog's request close return
        //   value, and dialog's request close source element.
        // - getEnabledState being to return true if dialog's enable close watcher for request close
        //   is true or dialog's computed closed-by state is not None; otherwise false.
        let close_watcher = InternalCloseWatcher::establish(
            no_gc,
            dialog.owner_document().window(),
            &InternalCloseWatcherOwner::Dialog(Dom::from_ref(self)),
        );
        self.close_watcher
            .safe_borrow_mut(no_gc)
            .replace(close_watcher);
    }

    /// <https://html.spec.whatwg.org/multipage/#close-the-dialog>
    pub(crate) fn close_the_dialog(
        &self,
        cx: &mut js::context::JSContext,
        result: Option<DOMString>,
        source: Option<DomRoot<Element>>,
    ) {
        let subject = self.upcast::<Element>();
        // Step 1. If subject does not have an open attribute, then return.
        if !subject.has_attribute(&local_name!("open")) {
            return;
        }

        // Step 2. Fire an event named beforetoggle, using ToggleEvent, with the oldState attribute initialized to "open", the newState attribute initialized to "closed", and the source attribute initialized to source at subject.
        let event = ToggleEvent::new(
            cx,
            &self.owner_window(),
            atom!("beforetoggle"),
            EventBubbles::DoesNotBubble,
            EventCancelable::NotCancelable,
            DOMString::from_static("open"),
            DOMString::from_static("closed"),
            source.as_deref(),
        );
        let event = event.upcast::<Event>();
        event.fire(cx, self.upcast::<EventTarget>());

        // Step 3. If subject does not have an open attribute, then return.
        if !subject.has_attribute(&local_name!("open")) {
            return;
        }

        // Step 4. Queue a dialog toggle event task given subject, "open", "closed", and source.
        self.queue_dialog_toggle_event_task("open", "closed", source);

        // Step 5. Remove subject's open attribute.
        subject.remove_attribute(cx, &ns!(), &local_name!("open"));
        subject.set_open_state(cx.no_gc(), false);

        // Step 6. If is modal of subject is true, then request an element to be removed from the
        // top layer given subject.
        // Step 7. Let wasModal be the value of subject's is modal flag.
        let was_modal = subject.state().contains(ElementState::MODAL);
        if was_modal {
            subject
                .owner_document()
                .request_an_element_to_be_removed_from_the_top_layer(cx.no_gc(), subject);
        }

        // Step 8. Set is modal of subject to false.
        self.upcast::<Element>().set_modal_state(cx.no_gc(), false);

        // Step 9. If result is not null, then set subject's returnValue attribute to result.
        if let Some(new_value) = result {
            *self.return_value.safe_borrow_mut(cx.no_gc()) = new_value.traced();
        }

        // Step 10. Set subject's request close return value to null.
        *self.request_close_return_value.safe_borrow_mut(cx) = None;

        // Step 11. Set subject's request close source element to null.
        self.request_close_source_element.set(None);

        // Step 12. If subject's previously focused element is not null, then:
        if let Some(element) = self
            .upcast::<HTMLElement>()
            .previously_focused_element(cx.no_gc())
        {
            // Step 12.1. Let element be subject's previously focused element.
            // Step 12.2. Set subject's previously focused element to null.
            self.upcast::<HTMLElement>()
                .set_previously_focused_element(None, cx.no_gc());

            // Step 12.3. If subject's node document's focused area of the document's DOM anchor is
            // a shadow-including inclusive descendant of subject, or wasModal is true, then run the
            // focusing steps for element; the viewport should not be scrolled by doing this step.
            //
            // Like Gecko, use the flat tree rather than a shadow-including traversal, so that an
            // element slotted into the dialog counts as inside it (as in other engines, see
            // https://github.com/web-platform-tests/wpt/pull/39579#issuecomment-2666758496).
            let subject_node = subject.upcast::<Node>();
            let document = subject.owner_document();
            let focused_node = document
                .focus_handler()
                .focused_area()
                .dom_anchor(&document);
            let focus_is_in_dialog = focused_node
                .inclusive_ancestors_in_flat_tree_unrooted(cx.no_gc())
                .any(|ancestor| &**ancestor == subject_node);
            if focus_is_in_dialog || was_modal {
                element
                    .upcast::<Node>()
                    .run_the_focusing_steps(cx, None, FocusTrigger::Script);
            }
        }

        // Step 13. Queue an element task on the user interaction task source given the subject element to fire an event named close at subject.
        let target = self.upcast::<EventTarget>();
        self.owner_global()
            .task_manager()
            .user_interaction_task_source()
            .queue_simple_event(target, atom!("close"));
    }

    /// <https://html.spec.whatwg.org/multipage/#dialog-request-close>
    pub(crate) fn request_to_close(
        &self,
        cx: &mut JSContext,
        return_value: Option<DOMString>,
        source: Option<DomRoot<Element>>,
    ) {
        let subject = self.upcast::<Element>();

        // Step 1. If subject does not have an open attribute, then return.
        if !subject.has_attribute(&local_name!("open")) {
            return;
        }

        // Step 2. If subject is not connected or subject's node document is not fully active, then
        // return.
        if !subject.is_connected() || !subject.owner_document().is_fully_active() {
            return;
        }

        // Step 3. Assert: subject's close watcher is not null.
        //
        // Clone the handle so the cell isn't borrowed while script runs: a `close` listener can
        // call `dialog.close()`, which runs `dialog_cleanup_steps` and nulls the close watcher.
        let close_watcher = self
            .close_watcher
            .borrow()
            .clone()
            .expect("Close watcher must always exist");

        // Step 4. Set subject's enable close watcher for request close to true.
        self.enable_close_watcher_for_request_close.set(true);

        // Step 5. Set subject's request close return value to returnValue.
        *self.request_close_return_value.safe_borrow_mut(cx) = return_value.map(DOMString::traced);

        // Step 6. Set subject's request close source element to source.
        self.request_close_source_element.set(source.as_deref());

        // Step 7. Request to close subject's close watcher with false.
        close_watcher.request_to_close(cx, false);

        // Step 8. Set subject's enable close watcher for request close to false.
        self.enable_close_watcher_for_request_close.set(false);
    }

    /// <https://html.spec.whatwg.org/multipage/#queue-a-dialog-toggle-event-task>
    pub(crate) fn queue_dialog_toggle_event_task(
        &self,
        old_state: &'static str,
        new_state: &'static str,
        source: Option<DomRoot<Element>>,
    ) {
        // Step 1. If element's dialog toggle task tracker is not null, then set oldState to the
        // tracker's old state, and remove the tracker's task from its task queue.
        let old_state = self.toggle_task_old_state.get().unwrap_or(old_state);
        self.toggle_task_old_state.set(Some(old_state));
        let counter = self.toggle_task_counter.get().wrapping_add(1);
        self.toggle_task_counter.set(counter);

        // Step 2. Queue an element task given the DOM manipulation task source and element to run the following steps:
        let this = Trusted::new(self);
        let trusted_source = source.map(|el| Trusted::new(&*el));

        self.owner_global()
            .task_manager()
            .dom_manipulation_task_source()
            .queue(task!(fire_toggle_event: move |cx| {
                let this = this.root();
                if counter != this.toggle_task_counter.get() {
                    return;
                }
                // Step 2.2. Set element's dialog toggle task tracker to null.
                this.toggle_task_old_state.set(None);

                let source = trusted_source.map(|s| s.root());

                // Step 2.1. Fire an event named toggle at element, using ToggleEvent, with the oldState attribute initialized to oldState, the newState attribute initialized to newState, and the source attribute initialized to source.
                let event = ToggleEvent::new(
                    cx,
                    &this.owner_window(),
                    atom!("toggle"),
                    EventBubbles::DoesNotBubble,
                    EventCancelable::NotCancelable,
                    DOMString::from_static(old_state),
                    DOMString::from_static(new_state),
                    source.as_deref(),
                );
                let event = event.upcast::<Event>();
                event.fire(cx, this.upcast::<EventTarget>());
            }));
    }

    /// The steps of `show()` and "show a modal dialog" that hide the popovers that are not
    /// ancestors of the dialog.
    fn hide_popovers_until_topmost_popover_ancestor(&self, cx: &mut JSContext) {
        let document = self.owner_document();
        let hide_until = topmost_popover_ancestor(cx.no_gc(), self.upcast(), None, false);
        document.hide_popovers_until(cx, hide_until.as_deref(), false, true);
    }

    /// <https://html.spec.whatwg.org/multipage/#dialog-focusing-steps>
    pub(crate) fn run_dialog_focusing_steps(&self, cx: &mut JSContext) {
        // TODO: Step 1. If the allow focus steps given subject's node document return false, then return.

        // Step 2. Let control be null.
        rooted!(&in(cx) let mut control = None);

        // Step 3. If subject has the autofocus attribute, then set control to subject.
        if self.upcast::<HTMLElement>().Autofocus() {
            control.set(
                self.upcast::<Node>()
                    .get_the_focusable_area(cx, FocusTrigger::Script),
            );
        }

        // Step 4. If control is null, then set control to the focus delegate of subject.
        if control.is_none() {
            control.set(self.upcast::<Node>().focus_delegate(cx, None));
        }

        // Step 5. If control is null, then set control to subject.
        if control.is_none() {
            control.set(
                self.upcast::<Node>()
                    .get_the_focusable_area(cx, FocusTrigger::Script),
            );
        }

        // Step 6. Run the focusing steps for control.
        // FIXME: Use the focusing step once they support a focusable area as an argument
        //
        // The focusing steps do nothing for an inert control. When a modal dialog has nothing to
        // focus, the element focused before is inert now, so clear the focus as Gecko does
        // (`FocusCandidate`), which leaves the body focused.
        let document = self.owner_document();
        let is_modal = self
            .upcast::<Element>()
            .state()
            .contains(ElementState::MODAL);
        match control.take() {
            Some(control) if !control.dom_anchor(&document).is_inert() => {
                document
                    .focus_handler()
                    .focus(cx, &control, FocusTrigger::Script);
            },
            _ if is_modal => {
                document
                    .focus_handler()
                    .focus(cx, &FocusableArea::Viewport, FocusTrigger::Script);
            },
            _ => {},
        }

        // Step 7. Let topDocument be control's node navigable's top-level traversable's active
        // document.
        // Step 8. If control's node document's origin is not the same as the origin of
        // topDocument, then return.
        // Step 9. Empty topDocument's autofocus candidates.
        // Step 10. Set topDocument's autofocus processed flag to true.
        //
        // Autofocus candidates are tracked per document so far.
        document.mark_autofocus_processed();
    }

    /// <https://html.spec.whatwg.org/multipage/#dialog-setup-steps>
    pub(crate) fn dialog_setup_steps(&self, no_gc: &NoGC) {
        // Step 1. Assert: subject has an open attribute.
        let subject = self.upcast::<Element>();
        assert!(subject.has_attribute(&local_name!("open")));

        // Step 2. Assert: subject is connected.
        assert!(subject.is_connected());

        // Step 3. Assert: subject's node document's open dialogs list does not contain subject.
        // Step 4. Add subject to subject's node document's open dialogs list.
        let document = subject.owner_document();
        let mut open_dialogs = document.dialog_data().open_dialogs.safe_borrow_mut(no_gc);
        debug_assert!(!open_dialogs.iter().any(|dialog| &**dialog == self));
        open_dialogs.push(Dom::from_ref(self));
        drop(open_dialogs);

        // Step 5. Set the dialog close watcher with subject.
        self.set_the_close_watcher(no_gc);
    }

    /// <https://html.spec.whatwg.org/multipage/#dialog-setup-steps>
    pub(crate) fn dialog_cleanup_steps(&self, no_gc: &NoGC) {
        // Step 1. Remove subject from subject's node document's open dialogs list.
        self.owner_document()
            .dialog_data()
            .open_dialogs
            .safe_borrow_mut(no_gc)
            .retain(|dialog| &**dialog != self);

        // Step 2. If subject's close watcher is not null, then:
        // Step 2.1. Destroy subject's close watcher.
        // Step 2.2. Set subject's close watcher to null.
        let close_watcher = self.close_watcher.safe_borrow_mut(no_gc).take();
        if let Some(close_watcher) = close_watcher {
            close_watcher.destroy(no_gc);
        }
    }

    pub(crate) fn close_watcher_close(&self, cx: &mut JSContext) {
        let return_value = self
            .request_close_return_value
            .borrow()
            .clone()
            .map(TracedDOMString::root);
        let source = self.request_close_source_element.get();
        self.close_the_dialog(cx, return_value, source)
    }

    pub(crate) fn close_watcher_enabled_state(&self) -> bool {
        self.enable_close_watcher_for_request_close.get() ||
            self.computed_closed_by_state() != ComputedClosedByState::None
    }

    /// <https://html.spec.whatwg.org/multipage/#computed-closed-by-state>
    fn computed_closed_by_state(&self) -> ComputedClosedByState {
        let element = self.upcast::<Element>();

        // Step 2. Return the state of dialog's closedby attribute.
        if let Some(closed_by) = element.get_attribute_string_ref(&local_name!("closedby")) {
            if closed_by.eq_ignore_ascii_case("any") {
                return ComputedClosedByState::Any;
            }
            if closed_by.eq_ignore_ascii_case("closerequest") {
                return ComputedClosedByState::CloseRequest;
            }
            if closed_by.eq_ignore_ascii_case("none") {
                return ComputedClosedByState::None;
            }
        }

        // Step 1. If the state of dialog's closedby attribute is Auto:
        // https://html.spec.whatwg.org/multipage/#attr-dialog-closedby-auto-state
        // > The closedby attribute's invalid value default and missing value default are both the
        // > Auto state.

        // Step 1.1. If dialog's is modal is true, then return Close Request.
        if element.state().contains(ElementState::MODAL) {
            return ComputedClosedByState::CloseRequest;
        }

        // Step 1.2. Return None.
        ComputedClosedByState::None
    }
}

impl HTMLDialogElementMethods<crate::DomTypeHolder> for HTMLDialogElement {
    // https://html.spec.whatwg.org/multipage/#dom-dialog-open
    make_bool_getter!(Open, "open");

    // https://html.spec.whatwg.org/multipage/#dom-dialog-open
    make_bool_setter!(SetOpen, "open");

    /// <https://html.spec.whatwg.org/multipage/#dom-dialog-returnvalue>
    fn ReturnValue(&self) -> DOMString {
        let return_value = self.return_value.borrow();
        return_value.clone().root()
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-dialog-returnvalue>
    fn SetReturnValue(&self, cx: &mut JSContext, return_value: DOMString) {
        *self.return_value.safe_borrow_mut(cx.no_gc()) = return_value.traced();
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-dialog-closedby>
    fn ClosedBy(&self) -> DOMString {
        // > The closedBy getter steps are to return the keyword
        // > corresponding to the computed closed-by state given this.
        match self.computed_closed_by_state() {
            ComputedClosedByState::Any => DOMString::from_static("any"),
            ComputedClosedByState::CloseRequest => DOMString::from_static("closerequest"),
            ComputedClosedByState::None => DOMString::from_static("none"),
        }
    }

    // https://html.spec.whatwg.org/multipage/#the-dialog-element:dom-dialog-closedby
    make_setter!(SetClosedBy, "closedby");

    /// <https://html.spec.whatwg.org/multipage/#dom-dialog-show>
    fn Show(&self, cx: &mut js::context::JSContext) -> ErrorResult {
        let element = self.upcast::<Element>();
        // Step 1. If this has an open attribute and is modal of this is false, then return.
        if element.has_attribute(&local_name!("open")) &&
            !element.state().contains(ElementState::MODAL)
        {
            return Ok(());
        }

        // Step 2. If this has an open attribute, then throw an "InvalidStateError" DOMException.
        if element.has_attribute(&local_name!("open")) {
            return Err(Error::InvalidState(Some(
                "Cannot call show() on an already open dialog.".into(),
            )));
        }

        // Step 3. If the result of firing an event named beforetoggle, using ToggleEvent, with the cancelable attribute initialized to true, the oldState attribute initialized to "closed", and the newState attribute initialized to "open" at this is false, then return.
        let event = ToggleEvent::new(
            cx,
            &self.owner_window(),
            atom!("beforetoggle"),
            EventBubbles::DoesNotBubble,
            EventCancelable::Cancelable,
            DOMString::from_static("closed"),
            DOMString::from_static("open"),
            None,
        );
        let event = event.upcast::<Event>();
        if !event.fire(cx, self.upcast::<EventTarget>()) {
            return Ok(());
        }

        // Step 4. If this has an open attribute, then return.
        if element.has_attribute(&local_name!("open")) {
            return Ok(());
        }

        // Step 5. Queue a dialog toggle event task given this, "closed", "open", and null.
        self.queue_dialog_toggle_event_task("closed", "open", None);

        // Step 6. Add an open attribute to this, whose value is the empty string.
        element.set_bool_attribute(cx, &local_name!("open"), true);
        element.set_open_state(cx.no_gc(), true);

        // Step 7. Set this's previously focused element to the focused element.
        self.upcast::<HTMLElement>().set_previously_focused_element(
            self.owner_document()
                .focus_handler()
                .focused_area()
                .element(),
            cx.no_gc(),
        );

        // Step 8. Let document be this's node document.
        // Step 9. Let hideUntil be the result of running topmost popover ancestor given this,
        // null, and false.
        // Step 10. Run hide popovers until given document, hideUntil, false, and true.
        self.hide_popovers_until_topmost_popover_ancestor(cx);

        // Step 11. Run the dialog focusing steps given this.
        self.run_dialog_focusing_steps(cx);
        Ok(())
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-dialog-showmodal>
    fn ShowModal(&self, cx: &mut js::context::JSContext) -> ErrorResult {
        // The showModal() method steps are to show a modal dialog given this and null.
        self.show_a_modal(cx, None)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-dialog-close>
    fn Close(&self, cx: &mut js::context::JSContext, return_value: Option<DOMString>) {
        // Step 1. If returnValue is not given, then set it to null.
        // Step 2. Close the dialog this with returnValue and null.
        self.close_the_dialog(cx, return_value, None);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-dialog-requestclose>
    fn RequestClose(&self, cx: &mut js::context::JSContext, return_value: Option<DOMString>) {
        // Step 1. If returnValue is not given, then set it to null.
        // Step 2. Request to close the dialog this with returnValue and null.
        self.request_to_close(cx, return_value, None);
    }
}

impl VirtualMethods for HTMLDialogElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<HTMLElement>() as &dyn VirtualMethods)
    }

    /// <https://html.spec.whatwg.org/multipage/#the-dialog-element:is-valid-command-steps>
    fn is_valid_command_steps(&self, command: CommandState) -> bool {
        // Step 1. If command is in the Close state, the Request Close state, or the
        // ShowModal state, then return true.
        if command == CommandState::Close ||
            command == CommandState::RequestClose ||
            command == CommandState::ShowModal
        {
            return true;
        }

        // Step 2. Return false.
        false
    }

    /// <https://html.spec.whatwg.org/multipage/#the-dialog-element:command-steps>
    fn command_steps(
        &self,
        cx: &mut js::context::JSContext,
        source: DomRoot<HTMLButtonElement>,
        command: CommandState,
    ) -> bool {
        if self
            .super_type()
            .unwrap()
            .command_steps(cx, source.clone(), command)
        {
            return true;
        }

        // Step 1. If element is in the popover showing state, then return.
        if self.upcast::<HTMLElement>().is_popover_showing() {
            return false;
        }
        let element = self.upcast::<Element>();

        // Step 2. If command is in the Close state and element has an open attribute, then
        // close the dialog element with source's optional value and source.
        if command == CommandState::Close && element.has_attribute(&local_name!("open")) {
            let button_element = DomRoot::from_ref(source.upcast::<Element>());
            self.close_the_dialog(cx, source.optional_value(), Some(button_element));
            return true;
        }

        // Step 3. If command is in the Request Close state and element has an open attribute,
        // then request to close the dialog element with source's optional value and source.
        if command == CommandState::RequestClose && element.has_attribute(&local_name!("open")) {
            let button_element = DomRoot::from_ref(source.upcast::<Element>());
            self.request_to_close(cx, source.optional_value(), Some(button_element));
            return true;
        }

        // Step 4. If command is the Show Modal state and element does not have an open attribute,
        // then show a modal dialog given element and source.
        if command == CommandState::ShowModal && !element.has_attribute(&local_name!("open")) {
            let button_element = DomRoot::from_ref(source.upcast::<Element>());
            let _ = self.show_a_modal(cx, Some(button_element));
            return true;
        }

        false
    }

    /// <https://html.spec.whatwg.org/multipage/#the-dialog-element:html-element-insertion-steps>
    fn bind_to_tree(&self, cx: &mut JSContext, context: &BindContext) {
        self.super_type().unwrap().bind_to_tree(cx, context);
        let inserted_node = self.upcast::<Element>();

        // Step 1. If insertedNode's node document is not fully active, then return.
        if !inserted_node.owner_document().is_fully_active() {
            return;
        }

        // Step 2. If insertedNode has an open attribute and is connected, then run the dialog setup
        // steps given insertedNode.
        if inserted_node.has_attribute(&local_name!("open")) && inserted_node.is_connected() {
            self.dialog_setup_steps(cx.no_gc());
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#the-dialog-element:html-element-removing-steps>
    fn unbind_from_tree(&self, cx: &mut js::context::JSContext, context: &UnbindContext) {
        self.super_type().unwrap().unbind_from_tree(cx, context);
        let removed_node = self.upcast::<Element>();

        // Step 1. If removedNode has an open attribute, then run the dialog cleanup steps given
        // removedNode.
        if removed_node.has_attribute(&local_name!("open")) {
            self.dialog_cleanup_steps(cx.no_gc());
        }

        // Step 2. If removedNode's node document's top layer contains removedNode, then remove an
        // element from the top layer immediately given removedNode.
        let document = removed_node.owner_document();
        if document.top_layer_contains(removed_node) {
            document.remove_an_element_from_the_top_layer_immediately(cx.no_gc(), removed_node);
        }

        // Step 3. Set is modal of removedNode to false.
        removed_node.set_modal_state(cx.no_gc(), false)
    }

    fn attribute_mutated(
        &self,
        cx: &mut js::context::JSContext,
        attr: AttrRef<'_>,
        mutation: AttributeMutation,
    ) {
        self.super_type()
            .unwrap()
            .attribute_mutated(cx, attr, mutation);

        // Step 1. If namespace is not null, then return.
        if *attr.namespace() != ns!() {
            return;
        }

        // Step 2. If localName is not open, then return.
        if attr.local_name() != &local_name!("open") {
            return;
        }

        // The open state follows the open attribute, however it is changed.
        self.upcast::<Element>()
            .set_open_state(cx.no_gc(), !mutation.is_removal());

        // Step 3. If value is null and oldValue is not null, then run the dialog cleanup steps
        // given element.
        if matches!(mutation, AttributeMutation::Removed) {
            self.dialog_cleanup_steps(cx.no_gc());
            return;
        }

        // Step 4. If element's node document is not fully active, then return.
        let element = self.upcast::<Element>();
        if !element.owner_document().is_fully_active() {
            return;
        }

        // Step 5. If element is not connected, then return.
        if !element.is_connected() {
            return;
        }

        // Step 6. If value is not null and oldValue is null, then run the dialog setup steps given
        // element.
        if matches!(mutation, AttributeMutation::Set(None)) {
            self.dialog_setup_steps(cx.no_gc());
        }
    }
}

/// The dialog-related state of a document.
#[derive(Default, JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct DocumentDialogData {
    /// <https://html.spec.whatwg.org/multipage/#open-dialogs-list>
    open_dialogs: DomRefCell<Vec<Dom<HTMLDialogElement>>>,
    /// <https://html.spec.whatwg.org/multipage/#dialog-pointerdown-target>
    pointerdown_target: MutNullableDom<HTMLDialogElement>,
}

/// <https://html.spec.whatwg.org/multipage/#light-dismiss-open-dialogs>
///
/// This is called for a trusted primary pointer press or release, before dispatching that
/// pointer event. `client_point` is the event's clientX and clientY.
pub(crate) fn light_dismiss_open_dialogs(
    cx: &mut JSContext,
    event_type: &str,
    target: &EventTarget,
    client_point: Point2D<f64, CSSPixel>,
) {
    // Step 1. Assert: event's isTrusted attribute is true.
    // Step 2. Let document be event's target's node document.
    let Some(target) = target.downcast::<Node>() else {
        return;
    };
    let document = target.owner_doc();

    // Step 3. If document's open dialogs list is empty, then return.
    if document.dialog_data().open_dialogs.borrow().is_empty() {
        return;
    }

    // Step 4. Let ancestor be the result of running nearest clicked dialog given event.
    let ancestor = nearest_clicked_dialog(cx, target, client_point);
    let dialog_data = document.dialog_data();

    // Step 5. If event's type is "pointerdown", then set document's dialog pointerdown target to
    // ancestor.
    if event_type == "pointerdown" {
        dialog_data.pointerdown_target.set(ancestor.as_deref());
        return;
    }

    // Step 6. If event's type is "pointerup", then:
    if event_type != "pointerup" {
        return;
    }

    // Step 6.1. Let sameTarget be true if ancestor is document's dialog pointerdown target.
    let same_target = ancestor == dialog_data.pointerdown_target.get();

    // Step 6.2. Set document's dialog pointerdown target to null.
    dialog_data.pointerdown_target.set(None);

    // Step 6.3. If sameTarget is false, then return.
    if !same_target {
        return;
    }

    // Step 6.4. Let topmostDialog be the last element of document's open dialogs list.
    let Some(topmost_dialog) = dialog_data
        .open_dialogs
        .borrow()
        .last()
        .map(|dialog| dialog.as_rooted())
    else {
        return;
    };

    // Step 6.5. If ancestor is topmostDialog, then return.
    if ancestor.as_ref() == Some(&topmost_dialog) {
        return;
    }

    // Step 6.6. If topmostDialog's computed closed-by state is not Any, then return.
    if topmost_dialog.computed_closed_by_state() != ComputedClosedByState::Any {
        return;
    }

    // Step 6.7. Assert: topmostDialog's close watcher is not null.
    // Step 6.8. Request to close topmostDialog's close watcher with false.
    //
    // Like Gecko, request to close the dialog itself with a null return value and source, so
    // that those of an earlier request to close that was canceled are not used.
    topmost_dialog.request_to_close(cx, None, None);
}

/// <https://html.spec.whatwg.org/multipage/#nearest-clicked-dialog>
fn nearest_clicked_dialog(
    cx: &mut JSContext,
    target: &Node,
    client_point: Point2D<f64, CSSPixel>,
) -> Option<DomRoot<HTMLDialogElement>> {
    // Step 1. Let target be event's target.
    // Step 2. If target is a dialog element, target has an open attribute, target's is modal is
    // true, and event's clientX and clientY are outside the bounds of target, then return null.
    //
    // A pointer event that hits the ::backdrop of a dialog targets the dialog element itself.
    if let Some(dialog) = target.downcast::<HTMLDialogElement>() {
        let element = dialog.upcast::<Element>();
        if element.has_attribute(&local_name!("open")) &&
            element.state().contains(ElementState::MODAL)
        {
            let bounds = target.border_box(cx).unwrap_or_default();
            let left = bounds.origin.x.to_f64_px();
            let top = bounds.origin.y.to_f64_px();
            let right = left + bounds.size.width.to_f64_px();
            let bottom = top + bounds.size.height.to_f64_px();
            if client_point.x < left ||
                client_point.x > right ||
                client_point.y < top ||
                client_point.y > bottom
            {
                return None;
            }
        }
    }

    // Step 3. Let currentNode be target.
    // Step 4. While currentNode is not null:
    // Step 4.1. If currentNode is a dialog element and currentNode has an open attribute, then
    // return currentNode.
    // Step 4.2. Set currentNode to currentNode's parent in the flat tree.
    // Step 5. Return null.
    target
        .inclusive_ancestors_in_flat_tree_unrooted(cx.no_gc())
        .filter_map(|ancestor| {
            ancestor
                .downcast::<HTMLDialogElement>()
                .map(DomRoot::from_ref)
        })
        .find(|dialog| {
            dialog
                .upcast::<Element>()
                .has_attribute(&local_name!("open"))
        })
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) enum ComputedClosedByState {
    Any,
    CloseRequest,
    None,
}
