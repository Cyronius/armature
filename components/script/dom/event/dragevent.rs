/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use euclid::Point2D;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::reflector::reflect_dom_object_with_proto;
use script_traits::MouseButtons;

use crate::dom::bindings::codegen::Bindings::DragEventBinding::{DragEventInit, DragEventMethods};
use crate::dom::bindings::codegen::Bindings::MouseEventBinding::MouseEventMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::datatransfer::DataTransfer;
use crate::dom::event::{Event, EventBubbles, EventCancelable};
use crate::dom::mouseevent::MouseEvent;
use crate::dom::window::Window;

/// <https://html.spec.whatwg.org/multipage/#dragevent>
#[dom_struct]
pub(crate) struct DragEvent {
    mouseevent: MouseEvent,
    data_transfer: MutNullableDom<DataTransfer>,
}

impl DragEvent {
    fn new_inherited() -> DragEvent {
        DragEvent {
            mouseevent: MouseEvent::new_inherited(),
            data_transfer: MutNullableDom::new(None),
        }
    }

    pub(crate) fn new_uninitialized(cx: &mut JSContext, window: &Window) -> DomRoot<DragEvent> {
        Self::new_uninitialized_with_proto(cx, window, None)
    }

    fn new_uninitialized_with_proto(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
    ) -> DomRoot<DragEvent> {
        reflect_dom_object_with_proto(cx, Box::new(DragEvent::new_inherited()), window, proto)
    }
}

impl DragEventMethods<crate::DomTypeHolder> for DragEvent {
    /// <https://html.spec.whatwg.org/multipage/#dom-dragevent-dragevent>
    fn Constructor(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        event_type: DOMString,
        init: &DragEventInit,
    ) -> DomRoot<DragEvent> {
        let mouse_init = &init.parent;
        let modifier_init = &mouse_init.parent;
        let ui_init = &modifier_init.parent;
        let scroll_offset = window.scroll_offset();
        let page_point = Point2D::new(
            scroll_offset.x as f64 + *mouse_init.clientX,
            scroll_offset.y as f64 + *mouse_init.clientY,
        );

        let event = DragEvent::new_uninitialized_with_proto(cx, window, proto);
        event.mouseevent.initialize_mouse_event(
            event_type.into(),
            EventBubbles::from(ui_init.parent.bubbles),
            EventCancelable::from(ui_init.parent.cancelable),
            ui_init.view.as_deref(),
            ui_init.detail,
            Point2D::new(*mouse_init.screenX, *mouse_init.screenY),
            Point2D::new(*mouse_init.clientX, *mouse_init.clientY),
            page_point,
            modifier_init.modifiers(),
            mouse_init.button.into(),
            MouseButtons::from_bits_retain(mouse_init.buttons),
            mouse_init.relatedTarget.as_deref(),
            None,
        );
        event
            .upcast::<Event>()
            .set_composed(ui_init.parent.composed);
        event.data_transfer.set(init.dataTransfer.as_deref());
        event
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-dragevent-datatransfer>
    fn GetDataTransfer(&self) -> Option<DomRoot<DataTransfer>> {
        self.data_transfer.get()
    }

    /// <https://dom.spec.whatwg.org/#dom-event-istrusted>
    fn IsTrusted(&self) -> bool {
        self.mouseevent.IsTrusted()
    }
}
