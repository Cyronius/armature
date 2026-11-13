/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use euclid::Angle;
use euclid::default::Transform3D;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::reflector::reflect_dom_object_with_proto;
use style::values::generics::calc::CalcType;

use super::cssnumericvalue::CSSNumericValue;
use super::csstransformcomponent::CSSTransformComponent;
use crate::dom::bindings::codegen::Bindings::CSSSkewYBinding::CSSSkewYMethods;
use crate::dom::bindings::error::Fallible;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{DomRoot, MutDom};
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSSkewY {
    css_transform_component: CSSTransformComponent,
    ay: MutDom<CSSNumericValue>,
}

impl CSSSkewY {
    pub(crate) fn serialize(&self) -> String {
        format!("skewY({})", self.ay.get().serialize())
    }

    pub(crate) fn matrix_data(&self, cx: &mut JSContext) -> Fallible<Transform3D<f64>> {
        let ax = 0.0;
        let ay = CSSTransformComponent::number(cx, &self.ay.get(), "deg")?;
        Ok(Transform3D::skew(Angle::degrees(ax), Angle::degrees(ay)))
    }
}

impl CSSSkewYMethods<crate::DomTypeHolder> for CSSSkewY {
    fn Constructor(
        cx: &mut JSContext,
        global: &crate::dom::window::Window,
        proto: Option<HandleObject>,
        ay: &CSSNumericValue,
    ) -> Fallible<DomRoot<Self>> {
        let global = global.upcast::<GlobalScope>();
        CSSTransformComponent::validate(ay, CalcType::Angle)?;
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                css_transform_component: CSSTransformComponent::new_inherited(true),
                ay: MutDom::new(ay),
            }),
            global,
            proto,
        ))
    }

    fn Ay(&self) -> DomRoot<CSSNumericValue> {
        self.ay.get()
    }
    fn SetAy(&self, value: &CSSNumericValue) -> Fallible<()> {
        CSSTransformComponent::validate(value, CalcType::Angle)?;
        self.ay.set(value);
        Ok(())
    }
}
