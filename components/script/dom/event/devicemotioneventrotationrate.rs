/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::DeviceMotionEventBinding::{
    DeviceMotionEventRotationRateInit, DeviceMotionEventRotationRateMethods,
};
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::DomRoot;
use crate::dom::globalscope::GlobalScope;

/// <https://w3c.github.io/deviceorientation/#devicemotion>
#[dom_struct]
pub(crate) struct DeviceMotionEventRotationRate {
    reflector_: Reflector,
    alpha: Option<Finite<f64>>,
    beta: Option<Finite<f64>>,
    gamma: Option<Finite<f64>>,
}

impl DeviceMotionEventRotationRate {
    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        init: &DeviceMotionEventRotationRateInit,
    ) -> DomRoot<DeviceMotionEventRotationRate> {
        reflect_dom_object(
            cx,
            Box::new(DeviceMotionEventRotationRate {
                reflector_: Reflector::new(),
                alpha: init.alpha,
                beta: init.beta,
                gamma: init.gamma,
            }),
            global,
        )
    }
}

impl DeviceMotionEventRotationRateMethods<crate::DomTypeHolder> for DeviceMotionEventRotationRate {
    /// <https://w3c.github.io/deviceorientation/#dom-devicemotioneventrotationrate-alpha>
    fn GetAlpha(&self) -> Option<Finite<f64>> {
        self.alpha
    }

    /// <https://w3c.github.io/deviceorientation/#dom-devicemotioneventrotationrate-beta>
    fn GetBeta(&self) -> Option<Finite<f64>> {
        self.beta
    }

    /// <https://w3c.github.io/deviceorientation/#dom-devicemotioneventrotationrate-gamma>
    fn GetGamma(&self) -> Option<Finite<f64>> {
        self.gamma
    }
}
