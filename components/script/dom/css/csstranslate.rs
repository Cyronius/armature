/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use euclid::default::Transform3D;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::reflector::reflect_dom_object_with_proto;
use style::values::generics::calc::CalcType;

use super::cssnumericvalue::CSSNumericValue;
use super::csstransformcomponent::CSSTransformComponent;
use super::cssunitvalue::CSSUnitValue;
use crate::dom::bindings::codegen::Bindings::CSSTranslateBinding::CSSTranslateMethods;
use crate::dom::bindings::error::Fallible;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{DomRoot, MutDom};
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSTranslate {
    css_transform_component: CSSTransformComponent,
    x: MutDom<CSSNumericValue>,
    y: MutDom<CSSNumericValue>,
    z: MutDom<CSSNumericValue>,
}

impl CSSTranslate {
    pub(crate) fn serialize(&self) -> String {
        if self.css_transform_component.is_2d() {
            format!(
                "translate({}, {})",
                self.x.get().serialize(),
                self.y.get().serialize()
            )
        } else {
            format!(
                "translate3d({}, {}, {})",
                self.x.get().serialize(),
                self.y.get().serialize(),
                self.z.get().serialize()
            )
        }
    }

    pub(crate) fn matrix_data(&self, cx: &mut JSContext) -> Fallible<Transform3D<f64>> {
        // Percentages and relative lengths require layout context to resolve.
        let x = CSSTransformComponent::number(cx, &self.x.get(), "px")?;
        let y = CSSTransformComponent::number(cx, &self.y.get(), "px")?;
        let z = CSSTransformComponent::number(cx, &self.z.get(), "px")?;
        Ok(Transform3D::translation(x, y, z))
    }
}

impl CSSTranslateMethods<crate::DomTypeHolder> for CSSTranslate {
    fn Constructor(
        cx: &mut JSContext,
        global: &crate::dom::window::Window,
        proto: Option<HandleObject>,
        x: &CSSNumericValue,
        y: &CSSNumericValue,
        z: Option<&CSSNumericValue>,
    ) -> Fallible<DomRoot<Self>> {
        let global = global.upcast::<GlobalScope>();
        CSSTransformComponent::validate_length_percentage(x)?;
        CSSTransformComponent::validate_length_percentage(y)?;
        let is_2d = z.is_none();
        let default_z = DomRoot::upcast(CSSUnitValue::new(cx, global, 0.0, "px"));
        let z = z.unwrap_or(&default_z);
        CSSTransformComponent::validate(z, CalcType::Length)?;
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                css_transform_component: CSSTransformComponent::new_inherited(is_2d),
                x: MutDom::new(x),
                y: MutDom::new(y),
                z: MutDom::new(z),
            }),
            global,
            proto,
        ))
    }

    fn X(&self) -> DomRoot<CSSNumericValue> {
        self.x.get()
    }
    fn Y(&self) -> DomRoot<CSSNumericValue> {
        self.y.get()
    }
    fn Z(&self) -> DomRoot<CSSNumericValue> {
        self.z.get()
    }

    fn SetX(&self, value: &CSSNumericValue) -> Fallible<()> {
        CSSTransformComponent::validate_length_percentage(value)?;
        self.x.set(value);
        Ok(())
    }
    fn SetY(&self, value: &CSSNumericValue) -> Fallible<()> {
        CSSTransformComponent::validate_length_percentage(value)?;
        self.y.set(value);
        Ok(())
    }
    fn SetZ(&self, value: &CSSNumericValue) -> Fallible<()> {
        CSSTransformComponent::validate(value, CalcType::Length)?;
        self.z.set(value);
        Ok(())
    }
}
