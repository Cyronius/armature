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
use crate::dom::bindings::codegen::Bindings::CSSSkewBinding::CSSSkewMethods;
use crate::dom::bindings::error::Fallible;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{DomRoot, MutDom};
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSSkew {
    css_transform_component: CSSTransformComponent,
    ax: MutDom<CSSNumericValue>,
    ay: MutDom<CSSNumericValue>,
}

impl CSSSkew {
    pub(crate) fn serialize(&self) -> String {
        format!(
            "skew({}, {})",
            self.ax.get().serialize(),
            self.ay.get().serialize()
        )
    }

    pub(crate) fn matrix_data(&self, cx: &mut JSContext) -> Fallible<Transform3D<f64>> {
        let ax = CSSTransformComponent::number(cx, &self.ax.get(), "deg")?;
        let ay = CSSTransformComponent::number(cx, &self.ay.get(), "deg")?;
        Ok(Transform3D::skew(Angle::degrees(ax), Angle::degrees(ay)))
    }
}

impl CSSSkewMethods<crate::DomTypeHolder> for CSSSkew {
    fn Constructor(
        cx: &mut JSContext,
        global: &crate::dom::window::Window,
        proto: Option<HandleObject>,
        ax: &CSSNumericValue,
        ay: &CSSNumericValue,
    ) -> Fallible<DomRoot<Self>> {
        let global = global.upcast::<GlobalScope>();
        CSSTransformComponent::validate(ax, CalcType::Angle)?;
        CSSTransformComponent::validate(ay, CalcType::Angle)?;
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                css_transform_component: CSSTransformComponent::new_inherited(true),
                ax: MutDom::new(ax),
                ay: MutDom::new(ay),
            }),
            global,
            proto,
        ))
    }

    fn Ax(&self) -> DomRoot<CSSNumericValue> {
        self.ax.get()
    }
    fn SetAx(&self, value: &CSSNumericValue) -> Fallible<()> {
        CSSTransformComponent::validate(value, CalcType::Angle)?;
        self.ax.set(value);
        Ok(())
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
