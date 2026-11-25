/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use js::conversions::ToJSValConvertible;
use js::jsapi::Heap;
use js::jsval::{BigIntValue, JSVal, UndefinedValue};
use js::rust::wrappers2::{BigIntFromUint64, ToBigInt};
use js::rust::{HandleObject, HandleValue, MutableHandleValue};
use script_bindings::cell::DomRefCell;
use script_bindings::domstring::TracedDOMString;
use script_bindings::inheritance::Castable;
use script_bindings::reflector::reflect_dom_object_with_proto;

use crate::dom::bindings::codegen::Bindings::ClipboardChangeEventBinding::{
    ClipboardChangeEventInit, ClipboardChangeEventMethods,
};
use crate::dom::bindings::codegen::Bindings::EventBinding::EventMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::frozenarray::CachedFrozenArray;
use crate::dom::bindings::root::{AsHandleValue, DomRoot};
use crate::dom::bindings::str::DOMString;
use crate::dom::event::Event;
use crate::dom::window::Window;

/// <https://w3c.github.io/clipboard-apis/#clipboardchangeevent>
#[dom_struct]
pub(crate) struct ClipboardChangeEvent {
    event: Event,
    types: DomRefCell<Vec<TracedDOMString>>,
    #[ignore_malloc_size_of = "Defined in rust-mozjs"]
    frozen_types: CachedFrozenArray,
    #[ignore_malloc_size_of = "Defined in rust-mozjs"]
    change_id: Heap<JSVal>,
}

impl ClipboardChangeEvent {
    #[expect(unsafe_code)]
    pub(super) fn for_change(
        cx: &mut JSContext,
        window: &Window,
        types: Vec<DOMString>,
        id: u128,
    ) -> Option<DomRoot<Self>> {
        rooted!(&in(cx) let mut id_string = UndefinedValue());
        DOMString::from(id.to_string()).to_jsval(cx, id_string.handle_mut());
        // SAFETY: The decimal string is rooted. Root the allocated BigInt
        // before the next JS operation.
        let bigint = unsafe { ToBigInt(cx, id_string.handle()).as_ref() }?;
        rooted!(&in(cx) let change_id = BigIntValue(bigint));
        let init = ClipboardChangeEventInit {
            types,
            ..Default::default()
        };
        let event = Self::new(
            cx,
            window,
            None,
            "clipboardchange".into(),
            &init,
            change_id.handle(),
        );
        event.upcast::<Event>().set_trusted(true);
        Some(event)
    }

    fn new(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        event_type: DOMString,
        init: &ClipboardChangeEventInit,
        change_id: HandleValue,
    ) -> DomRoot<Self> {
        let event = reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                event: Event::new_inherited(),
                types: Default::default(),
                frozen_types: CachedFrozenArray::new(),
                change_id: Heap::default(),
            }),
            window,
            proto,
        );
        *event.types.safe_borrow_mut(cx.no_gc()) = init
            .types
            .iter()
            .cloned()
            .map(|type_| type_.traced())
            .collect();
        event.change_id.set(change_id.get());
        let base = event.upcast::<Event>();
        base.init_event(
            event_type.into(),
            init.parent.bubbles,
            init.parent.cancelable,
        );
        base.set_composed(init.parent.composed);
        event
    }
}

impl ClipboardChangeEventMethods<crate::DomTypeHolder> for ClipboardChangeEvent {
    #[expect(unsafe_code)]
    fn Constructor(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        event_type: DOMString,
        init: &ClipboardChangeEventInit,
    ) -> Fallible<DomRoot<Self>> {
        // SAFETY: The incoming value is rooted by the binding. Root the returned
        // BigInt immediately before performing any additional JS operations.
        let bigint = unsafe {
            if init.changeId.get().is_undefined() {
                BigIntFromUint64(cx, 0)
            } else {
                ToBigInt(cx, init.changeId.as_handle_value())
            }
        };
        let bigint = unsafe { bigint.as_ref() }.ok_or(Error::JSFailed)?;
        rooted!(&in(cx) let change_id = BigIntValue(bigint));
        Ok(Self::new(
            cx,
            window,
            proto,
            event_type,
            init,
            change_id.handle(),
        ))
    }

    fn Types(&self, cx: &mut JSContext, retval: MutableHandleValue) {
        self.frozen_types.get_or_init(
            cx,
            || {
                self.types
                    .borrow()
                    .iter()
                    .cloned()
                    .map(|type_| type_.root())
                    .collect::<Vec<_>>()
            },
            retval,
        );
    }

    fn ChangeId(&self, mut retval: MutableHandleValue) {
        retval.set(self.change_id.get());
    }

    fn IsTrusted(&self) -> bool {
        self.event.IsTrusted()
    }
}
