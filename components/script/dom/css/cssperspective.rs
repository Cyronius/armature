/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use euclid::default::Transform3D;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::reflector::reflect_dom_object_with_proto;
use style::values::generics::calc::CalcType;

use super::csskeywordvalue::CSSKeywordValue;
use super::cssnumericvalue::CSSNumericValue;
use super::csstransformcomponent::CSSTransformComponent;
use crate::dom::bindings::codegen::Bindings::CSSKeywordValueBinding::CSSKeywordValueMethods;
use crate::dom::bindings::codegen::Bindings::CSSPerspectiveBinding::CSSPerspectiveMethods;
use crate::dom::bindings::codegen::UnionTypes::CSSNumericValueOrStringOrCSSKeywordValue;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{DomRoot, MutDom};
use crate::dom::css::cssstylevalue::CSSStyleValue;
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSPerspective {
    css_transform_component: CSSTransformComponent,
    length: MutDom<CSSStyleValue>,
}

impl CSSPerspective {
    fn rectify(
        cx: &mut JSContext,
        global: &GlobalScope,
        value: CSSNumericValueOrStringOrCSSKeywordValue,
    ) -> Fallible<DomRoot<CSSStyleValue>> {
        match value {
            CSSNumericValueOrStringOrCSSKeywordValue::CSSNumericValue(value) => {
                CSSTransformComponent::validate(&value, CalcType::Length)?;
                Ok(DomRoot::upcast(value))
            },
            CSSNumericValueOrStringOrCSSKeywordValue::CSSKeywordValue(value) => {
                if value.Value().to_string() != "none" {
                    return Err(Error::Type(c"Perspective keyword must be none".to_owned()));
                }
                Ok(DomRoot::upcast(value))
            },
            CSSNumericValueOrStringOrCSSKeywordValue::String(value) => {
                if value.to_string() != "none" {
                    return Err(Error::Type(c"Perspective keyword must be none".to_owned()));
                }
                Ok(DomRoot::upcast(CSSKeywordValue::new(
                    cx,
                    global,
                    value.into(),
                )))
            },
        }
    }

    pub(crate) fn serialize(&self) -> String {
        let value = self.length.get();
        if let Some(numeric) = value.downcast::<CSSNumericValue>() {
            format!("perspective({})", numeric.serialize())
        } else {
            "perspective(none)".to_owned()
        }
    }

    pub(crate) fn matrix_data(&self, cx: &mut JSContext) -> Fallible<Transform3D<f64>> {
        let value = self.length.get();
        let Some(numeric) = value.downcast::<CSSNumericValue>() else {
            return Ok(Transform3D::identity());
        };
        let length = CSSTransformComponent::number(cx, numeric, "px")?;
        let mut matrix = Transform3D::identity();
        matrix.m34 = -1.0 / length.max(1.0);
        Ok(matrix)
    }
}

impl CSSPerspectiveMethods<crate::DomTypeHolder> for CSSPerspective {
    fn Constructor(
        cx: &mut JSContext,
        global: &crate::dom::window::Window,
        proto: Option<HandleObject>,
        length: CSSNumericValueOrStringOrCSSKeywordValue,
    ) -> Fallible<DomRoot<Self>> {
        let global = global.upcast::<GlobalScope>();
        let length = Self::rectify(cx, global, length)?;
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                css_transform_component: CSSTransformComponent::new_inherited(false),
                length: MutDom::new(&length),
            }),
            global,
            proto,
        ))
    }

    fn Length(&self) -> CSSNumericValueOrStringOrCSSKeywordValue {
        let value = self.length.get();
        if let Some(numeric) = value.downcast::<CSSNumericValue>() {
            CSSNumericValueOrStringOrCSSKeywordValue::CSSNumericValue(DomRoot::from_ref(numeric))
        } else {
            CSSNumericValueOrStringOrCSSKeywordValue::CSSKeywordValue(DomRoot::from_ref(
                value.downcast::<CSSKeywordValue>().unwrap(),
            ))
        }
    }

    fn SetLength(
        &self,
        cx: &mut JSContext,
        value: CSSNumericValueOrStringOrCSSKeywordValue,
    ) -> Fallible<()> {
        let value = Self::rectify(cx, &self.global(), value)?;
        self.length.set(&value);
        Ok(())
    }
}
