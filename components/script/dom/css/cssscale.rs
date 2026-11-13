/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use euclid::default::Transform3D;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::reflector::reflect_dom_object_with_proto;
use style::values::generics::calc::CalcType;

use super::cssmathvalue::CSSMathValue;
use super::cssnumericvalue::CSSNumericValue;
use super::csstransformcomponent::CSSTransformComponent;
use super::cssunitvalue::CSSUnitValue;
use crate::dom::bindings::codegen::Bindings::CSSScaleBinding::CSSScaleMethods;
use crate::dom::bindings::codegen::UnionTypes::DoubleOrCSSNumericValue;
use crate::dom::bindings::error::Fallible;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{DomRoot, MutDom};
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSScale {
    css_transform_component: CSSTransformComponent,
    x: MutDom<CSSNumericValue>,
    y: MutDom<CSSNumericValue>,
    z: MutDom<CSSNumericValue>,
}

impl CSSScale {
    pub(crate) fn serialize(&self) -> String {
        if self.css_transform_component.is_2d() {
            format!(
                "scale({}, {})",
                self.x.get().serialize(),
                self.y.get().serialize()
            )
        } else {
            format!(
                "scale3d({}, {}, {})",
                self.x.get().serialize(),
                self.y.get().serialize(),
                self.z.get().serialize()
            )
        }
    }

    pub(crate) fn matrix_data(&self, cx: &mut JSContext) -> Fallible<Transform3D<f64>> {
        let x = CSSTransformComponent::number(cx, &self.x.get(), "number")?;
        let y = CSSTransformComponent::number(cx, &self.y.get(), "number")?;
        let z = CSSTransformComponent::number(cx, &self.z.get(), "number")?;
        Ok(Transform3D::scale(x, y, z))
    }
}

impl CSSScaleMethods<crate::DomTypeHolder> for CSSScale {
    fn Constructor(
        cx: &mut JSContext,
        global: &crate::dom::window::Window,
        proto: Option<HandleObject>,
        x: DoubleOrCSSNumericValue,
        y: DoubleOrCSSNumericValue,
        z: Option<DoubleOrCSSNumericValue>,
    ) -> Fallible<DomRoot<Self>> {
        let global = global.upcast::<GlobalScope>();
        let x = CSSMathValue::rectify(cx, global, x);
        let y = CSSMathValue::rectify(cx, global, y);
        CSSTransformComponent::validate(&x, CalcType::Number)?;
        CSSTransformComponent::validate(&y, CalcType::Number)?;
        let is_2d = z.is_none();
        let z = z
            .map(|value| CSSMathValue::rectify(cx, global, value))
            .unwrap_or_else(|| DomRoot::upcast(CSSUnitValue::new(cx, global, 1.0, "number")));
        CSSTransformComponent::validate(&z, CalcType::Number)?;
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                css_transform_component: CSSTransformComponent::new_inherited(is_2d),
                x: MutDom::new(&x),
                y: MutDom::new(&y),
                z: MutDom::new(&z),
            }),
            global,
            proto,
        ))
    }

    fn X(&self) -> DoubleOrCSSNumericValue {
        DoubleOrCSSNumericValue::CSSNumericValue(self.x.get())
    }
    fn Y(&self) -> DoubleOrCSSNumericValue {
        DoubleOrCSSNumericValue::CSSNumericValue(self.y.get())
    }
    fn Z(&self) -> DoubleOrCSSNumericValue {
        DoubleOrCSSNumericValue::CSSNumericValue(self.z.get())
    }

    fn SetX(&self, cx: &mut JSContext, value: DoubleOrCSSNumericValue) -> Fallible<()> {
        let value = CSSMathValue::rectify(cx, &self.global(), value);
        CSSTransformComponent::validate(&value, CalcType::Number)?;
        self.x.set(&value);
        Ok(())
    }
    fn SetY(&self, cx: &mut JSContext, value: DoubleOrCSSNumericValue) -> Fallible<()> {
        let value = CSSMathValue::rectify(cx, &self.global(), value);
        CSSTransformComponent::validate(&value, CalcType::Number)?;
        self.y.set(&value);
        Ok(())
    }
    fn SetZ(&self, cx: &mut JSContext, value: DoubleOrCSSNumericValue) -> Fallible<()> {
        let value = CSSMathValue::rectify(cx, &self.global(), value);
        CSSTransformComponent::validate(&value, CalcType::Number)?;
        self.z.set(&value);
        Ok(())
    }
}
