/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The drag-and-drop processing model for draggable elements, within one document.
//!
//! <https://html.spec.whatwg.org/multipage/#drag-and-drop-processing-model>

use euclid::Point2D;
use js::context::JSContext;
use script_bindings::cell::DomRefCell;
use script_traits::ConstellationInputEvent;
use style_traits::CSSPixel;

use crate::dom::bindings::codegen::Bindings::HTMLElementBinding::HTMLElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::datatransfer::DataTransfer;
use crate::dom::dragevent::DragEvent;
use crate::dom::element::Element;
use crate::dom::event::Event;
use crate::dom::eventtarget::EventTarget;
use crate::dom::html::htmlelement::HTMLElement;
use crate::dom::html::htmlinputelement::HTMLInputElement;
use crate::dom::html::htmlselectelement::HTMLSelectElement;
use crate::dom::html::htmltextareaelement::HTMLTextAreaElement;
use crate::dom::inputevent::HitTestResult;
use crate::dom::iterators::ShadowIncluding;
use crate::dom::node::Node;
use crate::dom::window::Window;
use crate::drag::drag_data_store::Mode;

/// How far the pointer has to move with the button down before a drag starts.
const DRAG_THRESHOLD: f32 = 3.0;

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) enum ElementDrag {
    /// The primary button went down on a draggable element but the pointer has not moved far yet.
    Pending {
        source: Dom<Element>,
        #[no_trace]
        start: Point2D<f32, CSSPixel>,
    },
    Active(ActiveDrag),
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct ActiveDrag {
    source: Dom<Element>,
    data_transfer: Dom<DataTransfer>,
    /// The element that last received a `dragenter`.
    current_target: Option<Dom<Element>>,
    /// <https://html.spec.whatwg.org/multipage/#current-target-element>
    target_accepts: bool,
    /// <https://html.spec.whatwg.org/multipage/#current-drag-operation>
    operation: &'static str,
}

/// The element that would be dragged when the pointer goes down on `node`.
pub(crate) fn draggable_element_for(node: &Node) -> Option<DomRoot<Element>> {
    for ancestor in node.inclusive_ancestors(ShadowIncluding::Yes) {
        // Pressing the pointer on a control that the user edits or picks from selects text or
        // opens it, which is not a drag of the element around it.
        let is_text_input = ancestor.downcast::<HTMLInputElement>().is_some_and(|input| {
            !matches!(
                input.input_type().as_str(),
                "button" | "submit" | "reset" | "image" | "checkbox" | "radio" | "range" | "color"
            )
        });
        if is_text_input ||
            ancestor.is::<HTMLTextAreaElement>() ||
            ancestor.is::<HTMLSelectElement>() ||
            ancestor.is_editable_or_editing_host()
        {
            return None;
        }
        if let Some(element) = ancestor.downcast::<HTMLElement>() &&
            element.Draggable()
        {
            return Some(DomRoot::from_ref(element.upcast::<Element>()));
        }
    }
    None
}

pub(crate) fn start_pending(
    state: &DomRefCell<Option<ElementDrag>>,
    source: &Element,
    start: Point2D<f32, CSSPixel>,
) {
    *state.borrow_mut() = Some(ElementDrag::Pending {
        source: Dom::from_ref(source),
        start,
    });
}

fn fire(
    cx: &mut JSContext,
    window: &Window,
    event_type: &'static str,
    target: &EventTarget,
    hit_test_result: &HitTestResult,
    input_event: &ConstellationInputEvent,
    data_transfer: &DataTransfer,
    related_target: Option<&EventTarget>,
) -> bool {
    DragEvent::new_for_drag_and_drop(
        cx,
        window,
        event_type,
        hit_test_result,
        input_event,
        data_transfer,
        related_target,
    )
    .upcast::<Event>()
    .fire(cx, target)
}

/// The operation a drag has when the target of the events says nothing else.
fn default_drop_effect(effect_allowed: &str) -> &'static str {
    match effect_allowed {
        "copy" | "copyLink" | "copyMove" | "all" | "uninitialized" => "copy",
        "link" => "link",
        "move" | "linkMove" => "move",
        _ => "none",
    }
}

/// <https://html.spec.whatwg.org/multipage/#current-drag-operation>
fn operation_for(effect_allowed: &str, drop_effect: &str) -> &'static str {
    match drop_effect {
        "copy" if matches!(
            effect_allowed,
            "uninitialized" | "copy" | "copyLink" | "copyMove" | "all"
        ) =>
        {
            "copy"
        },
        "link" if matches!(
            effect_allowed,
            "uninitialized" | "link" | "copyLink" | "linkMove" | "all"
        ) =>
        {
            "link"
        },
        "move" if matches!(
            effect_allowed,
            "uninitialized" | "move" | "copyMove" | "linkMove" | "all"
        ) =>
        {
            "move"
        },
        _ => "none",
    }
}

fn target_element_of(hit_test_result: &HitTestResult) -> Option<DomRoot<Element>> {
    hit_test_result
        .node
        .inclusive_ancestors(ShadowIncluding::Yes)
        .find_map(DomRoot::downcast::<Element>)
}

/// Handle a mouse move while the primary button is down. Returns `true` when a drag is in progress, in
/// which case the move is not delivered to the page as a mouse event.
pub(crate) fn handle_move(
    cx: &mut JSContext,
    window: &Window,
    state: &DomRefCell<Option<ElementDrag>>,
    hit_test_result: &HitTestResult,
    input_event: &ConstellationInputEvent,
) -> bool {
    let current = state.borrow_mut().take();
    let mut active = match current {
        None => return false,
        Some(ElementDrag::Pending { source, start }) => {
            if !input_event.primary_button_is_pressed() {
                return false;
            }
            let distance = hit_test_result.point_in_frame - start;
            if distance.x.abs() < DRAG_THRESHOLD && distance.y.abs() < DRAG_THRESHOLD {
                *state.borrow_mut() = Some(ElementDrag::Pending { source, start });
                return false;
            }
            // Step 1. Fire a DND event named dragstart at the source node.
            let source = DomRoot::from_ref(&*source);
            let data_transfer = DataTransfer::new_for_drag(cx, window);
            let not_canceled = fire(
                cx,
                window,
                "dragstart",
                source.upcast(),
                hit_test_result,
                input_event,
                &data_transfer,
                None,
            );
            if !not_canceled || !source.is_connected() {
                return false;
            }
            data_transfer.set_store_mode(Mode::Protected);
            ActiveDrag {
                source: Dom::from_ref(&source),
                data_transfer: Dom::from_ref(&data_transfer),
                current_target: None,
                target_accepts: false,
                operation: "none",
            }
        },
        Some(ElementDrag::Active(active)) => active,
    };

    if !input_event.primary_button_is_pressed() {
        // The button went up without us seeing it; finish the drag.
        finish(cx, window, active, hit_test_result, input_event);
        return true;
    }

    let data_transfer = DomRoot::from_ref(&*active.data_transfer);
    let source = DomRoot::from_ref(&*active.source);

    // Step 2. Fire a DND event named drag at the source node.
    fire(
        cx,
        window,
        "drag",
        source.upcast(),
        hit_test_result,
        input_event,
        &data_transfer,
        None,
    );

    // Step 3. Fire dragenter and dragover at the element under the pointer.
    if let Some(new_target) = target_element_of(hit_test_result) {
        let previous_target = active.current_target.as_ref().map(|t| DomRoot::from_ref(&**t));
        let target_changed = previous_target
            .as_ref()
            .is_none_or(|previous| *previous != new_target);
        let effect_allowed = data_transfer.effect_allowed_value();

        if target_changed {
            data_transfer.set_drop_effect_value(default_drop_effect(&effect_allowed));
            let accepted = !fire(
                cx,
                window,
                "dragenter",
                new_target.upcast(),
                hit_test_result,
                input_event,
                &data_transfer,
                previous_target.as_ref().map(|t| t.upcast()),
            );
            active.target_accepts = accepted;
            active.operation = if accepted {
                operation_for(&effect_allowed, &data_transfer.drop_effect_value())
            } else {
                "none"
            };
            if let Some(previous_target) = &previous_target {
                fire(
                    cx,
                    window,
                    "dragleave",
                    previous_target.upcast(),
                    hit_test_result,
                    input_event,
                    &data_transfer,
                    Some(new_target.upcast()),
                );
            }
            active.current_target = Some(Dom::from_ref(&new_target));
        }

        data_transfer.set_drop_effect_value(default_drop_effect(&effect_allowed));
        let accepted = !fire(
            cx,
            window,
            "dragover",
            new_target.upcast(),
            hit_test_result,
            input_event,
            &data_transfer,
            None,
        );
        active.target_accepts = accepted;
        active.operation = if accepted {
            operation_for(&effect_allowed, &data_transfer.drop_effect_value())
        } else {
            "none"
        };
    }

    *state.borrow_mut() = Some(ElementDrag::Active(active));
    true
}

/// Handle the primary button going up. Returns `true` when the release ended a drag, in which case it
/// is not delivered to the page as a mouse event.
pub(crate) fn handle_release(
    cx: &mut JSContext,
    window: &Window,
    state: &DomRefCell<Option<ElementDrag>>,
    hit_test_result: &HitTestResult,
    input_event: &ConstellationInputEvent,
) -> bool {
    match state.borrow_mut().take() {
        Some(ElementDrag::Active(active)) => {
            finish(cx, window, active, hit_test_result, input_event);
            true
        },
        _ => false,
    }
}

/// <https://html.spec.whatwg.org/multipage/#drag-and-drop-processing-model>, the steps run when the
/// user ends the drag.
fn finish(
    cx: &mut JSContext,
    window: &Window,
    mut active: ActiveDrag,
    hit_test_result: &HitTestResult,
    input_event: &ConstellationInputEvent,
) {
    let data_transfer = DomRoot::from_ref(&*active.data_transfer);
    let source = DomRoot::from_ref(&*active.source);
    let target = active.current_target.as_ref().map(|t| DomRoot::from_ref(&**t));

    match target {
        // Step 1. If the current target element is a DOM element that accepts the drop and the
        // current drag operation is not "none", fire a DND event named drop at it.
        Some(target) if active.target_accepts && active.operation != "none" => {
            data_transfer.set_drop_effect_value(active.operation);
            data_transfer.set_store_mode(Mode::ReadOnly);
            fire(
                cx,
                window,
                "drop",
                target.upcast(),
                hit_test_result,
                input_event,
                &data_transfer,
                None,
            );
        },
        // Otherwise, fire a DND event named dragleave at the current target element.
        Some(target) => {
            active.operation = "none";
            fire(
                cx,
                window,
                "dragleave",
                target.upcast(),
                hit_test_result,
                input_event,
                &data_transfer,
                None,
            );
        },
        None => active.operation = "none",
    }

    // Step 2. Fire a DND event named dragend at the source node.
    data_transfer.set_store_mode(Mode::Protected);
    data_transfer.set_drop_effect_value(active.operation);
    fire(
        cx,
        window,
        "dragend",
        source.upcast(),
        hit_test_result,
        input_event,
        &data_transfer,
        None,
    );
}
