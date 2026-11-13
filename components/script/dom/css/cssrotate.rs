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

use super::cssmathvalue::CSSMathValue;
use super::cssnumericvalue::CSSNumericValue;
use super::csstransformcomponent::CSSTransformComponent;
use super::cssunitvalue::CSSUnitValue;
use crate::dom::bindings::codegen::Bindings::CSSRotateBinding::CSSRotateMethods;
use crate::dom::bindings::codegen::UnionTypes::DoubleOrCSSNumericValue;
use crate::dom::bindings::error::Fallible;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{DomRoot, MutDom};
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSRotate {
    css_transform_component: CSSTransformComponent,
    x: MutDom<CSSNumericValue>,
    y: MutDom<CSSNumericValue>,
    z: MutDom<CSSNumericValue>,
    angle: MutDom<CSSNumericValue>,
}

impl CSSRotate {
    fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        x: DomRoot<CSSNumericValue>,
        y: DomRoot<CSSNumericValue>,
        z: DomRoot<CSSNumericValue>,
        angle: DomRoot<CSSNumericValue>,
        is_2d: bool,
    ) -> Fallible<DomRoot<Self>> {
        for axis in [&x, &y, &z] {
            CSSTransformComponent::validate(axis, CalcType::Number)?;
        }
        CSSTransformComponent::validate(&angle, CalcType::Angle)?;
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                css_transform_component: CSSTransformComponent::new_inherited(is_2d),
                x: MutDom::new(&x),
                y: MutDom::new(&y),
                z: MutDom::new(&z),
                angle: MutDom::new(&angle),
            }),
            global,
            proto,
        ))
    }

    pub(crate) fn serialize(&self) -> String {
        if self.css_transform_component.is_2d() {
            format!("rotate({})", self.angle.get().serialize())
        } else {
            format!(
                "rotate3d({}, {}, {}, {})",
                self.x.get().serialize(),
                self.y.get().serialize(),
                self.z.get().serialize(),
                self.angle.get().serialize()
            )
        }
    }

    pub(crate) fn matrix_data(&self, cx: &mut JSContext) -> Fallible<Transform3D<f64>> {
        let (x, y, z) = if self.css_transform_component.is_2d() {
            (0.0, 0.0, 1.0)
        } else {
            (
                CSSTransformComponent::number(cx, &self.x.get(), "number")?,
                CSSTransformComponent::number(cx, &self.y.get(), "number")?,
                CSSTransformComponent::number(cx, &self.z.get(), "number")?,
            )
        };
        let angle = CSSTransformComponent::number(cx, &self.angle.get(), "deg")?;
        let norm = (x * x + y * y + z * z).sqrt();
        if norm == 0.0 {
            return Ok(Transform3D::identity());
        }
        Ok(Transform3D::rotation(
            x / norm,
            y / norm,
            z / norm,
            Angle::degrees(angle),
        ))
    }
}

impl CSSRotateMethods<crate::DomTypeHolder> for CSSRotate {
    fn Constructor(
        cx: &mut JSContext,
        global: &crate::dom::window::Window,
        proto: Option<HandleObject>,
        angle: &CSSNumericValue,
    ) -> Fallible<DomRoot<Self>> {
        let global = global.upcast::<GlobalScope>();
        let zero: DomRoot<CSSNumericValue> =
            DomRoot::upcast(CSSUnitValue::new(cx, global, 0.0, "number"));
        let one: DomRoot<CSSNumericValue> =
            DomRoot::upcast(CSSUnitValue::new(cx, global, 1.0, "number"));
        Self::new(
            cx,
            global,
            proto,
            zero.clone(),
            zero,
            one,
            DomRoot::from_ref(angle),
            true,
        )
    }

    fn Constructor_(
        cx: &mut JSContext,
        global: &crate::dom::window::Window,
        proto: Option<HandleObject>,
        x: DoubleOrCSSNumericValue,
        y: DoubleOrCSSNumericValue,
        z: DoubleOrCSSNumericValue,
        angle: &CSSNumericValue,
    ) -> Fallible<DomRoot<Self>> {
        let global = global.upcast::<GlobalScope>();
        let x = CSSMathValue::rectify(cx, global, x);
        let y = CSSMathValue::rectify(cx, global, y);
        let z = CSSMathValue::rectify(cx, global, z);
        Self::new(cx, global, proto, x, y, z, DomRoot::from_ref(angle), false)
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
    fn Angle(&self) -> DomRoot<CSSNumericValue> {
        self.angle.get()
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
    fn SetAngle(&self, value: &CSSNumericValue) -> Fallible<()> {
        CSSTransformComponent::validate(value, CalcType::Angle)?;
        self.angle.set(value);
        Ok(())
    }
}
