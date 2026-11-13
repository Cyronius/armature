/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use euclid::default::Transform3D;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::cell::DomRefCell;
use script_bindings::reflector::reflect_dom_object_with_proto;

use super::cssstylevalue::CSSStyleValue;
use super::csstransformcomponent::CSSTransformComponent;
use crate::dom::bindings::codegen::Bindings::CSSTransformValueBinding::CSSTransformValueMethods;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::dommatrix::DOMMatrix;
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSTransformValue {
    css_style_value: CSSStyleValue,
    transforms: DomRefCell<Vec<Dom<CSSTransformComponent>>>,
}

impl CSSTransformValue {
    pub(crate) fn serialize(&self) -> String {
        self.transforms
            .borrow()
            .iter()
            .map(|component| component.serialize())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl CSSTransformValueMethods<crate::DomTypeHolder> for CSSTransformValue {
    fn Constructor(
        cx: &mut JSContext,
        global: &crate::dom::window::Window,
        proto: Option<HandleObject>,
        transforms: Vec<DomRoot<CSSTransformComponent>>,
    ) -> Fallible<DomRoot<Self>> {
        let global = global.upcast::<GlobalScope>();
        if transforms.is_empty() {
            return Err(Error::Type(
                c"CSSTransformValue needs at least one component".to_owned(),
            ));
        }
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                css_style_value: CSSStyleValue::new_inherited(String::new()),
                transforms: DomRefCell::new(
                    transforms
                        .iter()
                        .map(|value| Dom::from_ref(&**value))
                        .collect(),
                ),
            }),
            global,
            proto,
        ))
    }

    fn Length(&self) -> u32 {
        self.transforms.borrow().len() as u32
    }

    fn IndexedGetter(&self, index: u32) -> Option<DomRoot<CSSTransformComponent>> {
        self.transforms
            .borrow()
            .get(index as usize)
            .map(Dom::as_rooted)
    }

    fn IndexedSetter(&self, index: u32, value: &CSSTransformComponent) -> ErrorResult {
        let mut transforms = self.transforms.borrow_mut();
        if index as usize > transforms.len() {
            return Err(Error::Range(
                c"CSSTransformValue index is out of range".to_owned(),
            ));
        }
        if index as usize == transforms.len() {
            transforms.push(Dom::from_ref(value));
        } else {
            transforms[index as usize] = Dom::from_ref(value);
        }
        Ok(())
    }

    fn Is2D(&self) -> bool {
        self.transforms
            .borrow()
            .iter()
            .all(|transform| transform.is_2d())
    }

    fn ToMatrix(&self, cx: &mut JSContext) -> Fallible<DomRoot<DOMMatrix>> {
        let mut matrix = Transform3D::identity();
        for component in self.transforms.borrow().iter() {
            matrix = component.matrix_data(cx)?.then(&matrix);
        }
        Ok(DOMMatrix::new(cx, &self.global(), self.Is2D(), matrix))
    }
}
