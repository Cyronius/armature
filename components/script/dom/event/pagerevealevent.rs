/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::{HandleObject, MutableHandleObject};
use script_bindings::reflector::reflect_dom_object_with_proto;
use stylo_atoms::Atom;

use crate::dom::bindings::codegen::Bindings::EventBinding::EventMethods;
use crate::dom::bindings::codegen::Bindings::PageRevealEventBinding::{
    PageRevealEventInit, PageRevealEventMethods,
};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::event::Event;
use crate::dom::window::Window;

/// <https://html.spec.whatwg.org/multipage/#pagerevealevent>
#[dom_struct]
pub(crate) struct PageRevealEvent {
    event: Event,
}

impl PageRevealEvent {
    fn new_inherited() -> PageRevealEvent {
        PageRevealEvent {
            event: Event::new_inherited(),
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        type_: Atom,
        bubbles: bool,
        cancelable: bool,
    ) -> DomRoot<PageRevealEvent> {
        let event = reflect_dom_object_with_proto(
            cx,
            Box::new(PageRevealEvent::new_inherited()),
            window,
            proto,
        );
        event.upcast::<Event>().init_event(type_, bubbles, cancelable);
        event
    }
}

impl PageRevealEventMethods<crate::DomTypeHolder> for PageRevealEvent {
    /// <https://html.spec.whatwg.org/multipage/#dom-pagerevealevent>
    fn Constructor(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        type_: DOMString,
        init: &PageRevealEventInit,
    ) -> DomRoot<PageRevealEvent> {
        PageRevealEvent::new(
            cx,
            window,
            proto,
            Atom::from(type_),
            init.parent.bubbles,
            init.parent.cancelable,
        )
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-pagerevealevent-viewtransition>
    fn GetViewTransition(&self, mut rval: MutableHandleObject) {
        rval.set(std::ptr::null_mut());
    }

    /// <https://dom.spec.whatwg.org/#dom-event-istrusted>
    fn IsTrusted(&self) -> bool {
        self.event.IsTrusted()
    }
}
