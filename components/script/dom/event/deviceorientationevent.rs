/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::reflector::reflect_dom_object_with_proto;

use crate::dom::bindings::codegen::Bindings::DeviceOrientationEventBinding::{
    DeviceOrientationEventInit, DeviceOrientationEventMethods,
};
use crate::dom::bindings::codegen::Bindings::EventBinding::EventMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::event::Event;
use crate::dom::window::Window;

/// <https://w3c.github.io/deviceorientation/#deviceorientation>
#[dom_struct]
pub(crate) struct DeviceOrientationEvent {
    event: Event,
    alpha: Option<Finite<f64>>,
    beta: Option<Finite<f64>>,
    gamma: Option<Finite<f64>>,
    absolute: bool,
}

impl DeviceOrientationEvent {
    fn new_inherited(
        alpha: Option<Finite<f64>>,
        beta: Option<Finite<f64>>,
        gamma: Option<Finite<f64>>,
        absolute: bool,
    ) -> DeviceOrientationEvent {
        DeviceOrientationEvent {
            event: Event::new_inherited(),
            alpha,
            beta,
            gamma,
            absolute,
        }
    }

    pub(crate) fn new_uninitialized(
        cx: &mut JSContext,
        window: &Window,
    ) -> DomRoot<DeviceOrientationEvent> {
        reflect_dom_object_with_proto(
            cx,
            Box::new(DeviceOrientationEvent::new_inherited(
                None, None, None, false,
            )),
            window,
            None,
        )
    }
}

impl DeviceOrientationEventMethods<crate::DomTypeHolder> for DeviceOrientationEvent {
    /// <https://w3c.github.io/deviceorientation/#dom-deviceorientationevent-deviceorientationevent>
    fn Constructor(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        event_type: DOMString,
        init: &DeviceOrientationEventInit,
    ) -> DomRoot<DeviceOrientationEvent> {
        let event = reflect_dom_object_with_proto(
            cx,
            Box::new(DeviceOrientationEvent::new_inherited(
                init.alpha,
                init.beta,
                init.gamma,
                init.absolute,
            )),
            window,
            proto,
        );
        let base = event.upcast::<Event>();
        base.init_event(
            event_type.into(),
            init.parent.bubbles,
            init.parent.cancelable,
        );
        base.set_composed(init.parent.composed);
        event
    }

    /// <https://w3c.github.io/deviceorientation/#dom-deviceorientationevent-alpha>
    fn GetAlpha(&self) -> Option<Finite<f64>> {
        self.alpha
    }

    /// <https://w3c.github.io/deviceorientation/#dom-deviceorientationevent-beta>
    fn GetBeta(&self) -> Option<Finite<f64>> {
        self.beta
    }

    /// <https://w3c.github.io/deviceorientation/#dom-deviceorientationevent-gamma>
    fn GetGamma(&self) -> Option<Finite<f64>> {
        self.gamma
    }

    /// <https://w3c.github.io/deviceorientation/#dom-deviceorientationevent-absolute>
    fn Absolute(&self) -> bool {
        self.absolute
    }

    /// <https://dom.spec.whatwg.org/#dom-event-istrusted>
    fn IsTrusted(&self) -> bool {
        self.event.IsTrusted()
    }
}
