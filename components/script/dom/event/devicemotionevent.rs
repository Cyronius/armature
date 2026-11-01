/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::reflector::reflect_dom_object_with_proto;

use crate::dom::bindings::codegen::Bindings::DeviceMotionEventBinding::{
    DeviceMotionEventInit, DeviceMotionEventMethods,
};
use crate::dom::bindings::codegen::Bindings::EventBinding::EventMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::devicemotioneventacceleration::DeviceMotionEventAcceleration;
use crate::dom::devicemotioneventrotationrate::DeviceMotionEventRotationRate;
use crate::dom::event::Event;
use crate::dom::window::Window;

/// <https://w3c.github.io/deviceorientation/#devicemotion>
#[dom_struct]
pub(crate) struct DeviceMotionEvent {
    event: Event,
    acceleration: MutNullableDom<DeviceMotionEventAcceleration>,
    acceleration_including_gravity: MutNullableDom<DeviceMotionEventAcceleration>,
    rotation_rate: MutNullableDom<DeviceMotionEventRotationRate>,
    interval: Finite<f64>,
}

impl DeviceMotionEvent {
    fn new_inherited(interval: Finite<f64>) -> DeviceMotionEvent {
        DeviceMotionEvent {
            event: Event::new_inherited(),
            acceleration: MutNullableDom::new(None),
            acceleration_including_gravity: MutNullableDom::new(None),
            rotation_rate: MutNullableDom::new(None),
            interval,
        }
    }

    pub(crate) fn new_uninitialized(
        cx: &mut JSContext,
        window: &Window,
    ) -> DomRoot<DeviceMotionEvent> {
        reflect_dom_object_with_proto(
            cx,
            Box::new(DeviceMotionEvent::new_inherited(Finite::wrap(0.))),
            window,
            None,
        )
    }
}

impl DeviceMotionEventMethods<crate::DomTypeHolder> for DeviceMotionEvent {
    /// <https://w3c.github.io/deviceorientation/#dom-devicemotionevent-devicemotionevent>
    fn Constructor(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        event_type: DOMString,
        init: &DeviceMotionEventInit,
    ) -> DomRoot<DeviceMotionEvent> {
        let event = reflect_dom_object_with_proto(
            cx,
            Box::new(DeviceMotionEvent::new_inherited(init.interval)),
            window,
            proto,
        );
        let global = event.global();
        if let Some(acceleration) = &init.acceleration {
            event
                .acceleration
                .set(Some(&DeviceMotionEventAcceleration::new(
                    cx,
                    &global,
                    acceleration,
                )));
        }
        if let Some(acceleration) = &init.accelerationIncludingGravity {
            event
                .acceleration_including_gravity
                .set(Some(&DeviceMotionEventAcceleration::new(
                    cx,
                    &global,
                    acceleration,
                )));
        }
        if let Some(rotation_rate) = &init.rotationRate {
            event
                .rotation_rate
                .set(Some(&DeviceMotionEventRotationRate::new(
                    cx,
                    &global,
                    rotation_rate,
                )));
        }
        let base = event.upcast::<Event>();
        base.init_event(
            event_type.into(),
            init.parent.bubbles,
            init.parent.cancelable,
        );
        base.set_composed(init.parent.composed);
        event
    }

    /// <https://w3c.github.io/deviceorientation/#dom-devicemotionevent-acceleration>
    fn GetAcceleration(&self) -> Option<DomRoot<DeviceMotionEventAcceleration>> {
        self.acceleration.get()
    }

    /// <https://w3c.github.io/deviceorientation/#dom-devicemotionevent-accelerationincludinggravity>
    fn GetAccelerationIncludingGravity(&self) -> Option<DomRoot<DeviceMotionEventAcceleration>> {
        self.acceleration_including_gravity.get()
    }

    /// <https://w3c.github.io/deviceorientation/#dom-devicemotionevent-rotationrate>
    fn GetRotationRate(&self) -> Option<DomRoot<DeviceMotionEventRotationRate>> {
        self.rotation_rate.get()
    }

    /// <https://w3c.github.io/deviceorientation/#dom-devicemotionevent-interval>
    fn Interval(&self) -> Finite<f64> {
        self.interval
    }

    /// <https://dom.spec.whatwg.org/#dom-event-istrusted>
    fn IsTrusted(&self) -> bool {
        self.event.IsTrusted()
    }
}
