/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::DeviceMotionEventBinding::{
    DeviceMotionEventAccelerationInit, DeviceMotionEventAccelerationMethods,
};
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::DomRoot;
use crate::dom::globalscope::GlobalScope;

/// <https://w3c.github.io/deviceorientation/#devicemotion>
#[dom_struct]
pub(crate) struct DeviceMotionEventAcceleration {
    reflector_: Reflector,
    x: Option<Finite<f64>>,
    y: Option<Finite<f64>>,
    z: Option<Finite<f64>>,
}

impl DeviceMotionEventAcceleration {
    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        init: &DeviceMotionEventAccelerationInit,
    ) -> DomRoot<DeviceMotionEventAcceleration> {
        reflect_dom_object(
            cx,
            Box::new(DeviceMotionEventAcceleration {
                reflector_: Reflector::new(),
                x: init.x,
                y: init.y,
                z: init.z,
            }),
            global,
        )
    }
}

impl DeviceMotionEventAccelerationMethods<crate::DomTypeHolder> for DeviceMotionEventAcceleration {
    /// <https://w3c.github.io/deviceorientation/#dom-devicemotioneventacceleration-x>
    fn GetX(&self) -> Option<Finite<f64>> {
        self.x
    }

    /// <https://w3c.github.io/deviceorientation/#dom-devicemotioneventacceleration-y>
    fn GetY(&self) -> Option<Finite<f64>> {
        self.y
    }

    /// <https://w3c.github.io/deviceorientation/#dom-devicemotioneventacceleration-z>
    fn GetZ(&self) -> Option<Finite<f64>> {
        self.z
    }
}
