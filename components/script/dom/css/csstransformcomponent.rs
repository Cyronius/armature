/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use euclid::default::Transform3D;
use js::context::JSContext;
use script_bindings::reflector::Reflector;
use style::typed_om::numeric_type::NumericBaseType;
use style::values::generics::calc::CalcType;

use super::cssmatrixcomponent::CSSMatrixComponent;
use super::cssnumericvalue::CSSNumericValue;
use super::cssperspective::CSSPerspective;
use super::cssrotate::CSSRotate;
use super::cssscale::CSSScale;
use super::cssskew::CSSSkew;
use super::cssskewx::CSSSkewX;
use super::cssskewy::CSSSkewY;
use super::csstranslate::CSSTranslate;
use crate::dom::bindings::codegen::Bindings::CSSNumericValueBinding::CSSNumericValueMethods;
use crate::dom::bindings::codegen::Bindings::CSSTransformComponentBinding::CSSTransformComponentMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::{DOMString, USVString};
use crate::dom::dommatrix::DOMMatrix;

#[dom_struct]
pub(crate) struct CSSTransformComponent {
    reflector_: Reflector,
    is_2d: Cell<bool>,
}

impl CSSTransformComponent {
    pub(crate) fn validate(value: &CSSNumericValue, kind: CalcType) -> Fallible<()> {
        let numeric_type = value.numeric_type();
        let matches = numeric_type.as_calc_type() == Ok(kind);
        if matches {
            Ok(())
        } else {
            Err(Error::Type(
                c"Incompatible transform component value".to_owned(),
            ))
        }
    }

    pub(crate) fn validate_length_percentage(value: &CSSNumericValue) -> Fallible<()> {
        let hinted = value
            .numeric_type()
            .with_percent_hint(NumericBaseType::Length);
        if hinted.as_calc_type() == Ok(CalcType::Length) {
            Ok(())
        } else {
            Err(Error::Type(c"Expected a length or percentage".to_owned()))
        }
    }

    pub(crate) fn number(cx: &mut JSContext, value: &CSSNumericValue, unit: &str) -> Fallible<f64> {
        let converted = value.To(cx, USVString::from(unit.to_owned()))?;
        Ok(converted.numeric_value())
    }
    pub(crate) fn new_inherited(is_2d: bool) -> Self {
        Self {
            reflector_: Reflector::new(),
            is_2d: Cell::new(is_2d),
        }
    }

    pub(crate) fn is_2d(&self) -> bool {
        self.is_2d.get()
    }

    pub(crate) fn serialize(&self) -> String {
        if let Some(value) = self.downcast::<CSSTranslate>() {
            value.serialize()
        } else if let Some(value) = self.downcast::<CSSRotate>() {
            value.serialize()
        } else if let Some(value) = self.downcast::<CSSScale>() {
            value.serialize()
        } else if let Some(value) = self.downcast::<CSSSkew>() {
            value.serialize()
        } else if let Some(value) = self.downcast::<CSSSkewX>() {
            value.serialize()
        } else if let Some(value) = self.downcast::<CSSSkewY>() {
            value.serialize()
        } else if let Some(value) = self.downcast::<CSSPerspective>() {
            value.serialize()
        } else if let Some(value) = self.downcast::<CSSMatrixComponent>() {
            value.serialize()
        } else {
            unreachable!("Every transform component has a concrete type")
        }
    }

    pub(crate) fn matrix_data(&self, cx: &mut JSContext) -> Fallible<Transform3D<f64>> {
        let matrix = if let Some(value) = self.downcast::<CSSTranslate>() {
            value.matrix_data(cx)
        } else if let Some(value) = self.downcast::<CSSRotate>() {
            value.matrix_data(cx)
        } else if let Some(value) = self.downcast::<CSSScale>() {
            value.matrix_data(cx)
        } else if let Some(value) = self.downcast::<CSSSkew>() {
            value.matrix_data(cx)
        } else if let Some(value) = self.downcast::<CSSSkewX>() {
            value.matrix_data(cx)
        } else if let Some(value) = self.downcast::<CSSSkewY>() {
            value.matrix_data(cx)
        } else if let Some(value) = self.downcast::<CSSPerspective>() {
            value.matrix_data(cx)
        } else if let Some(value) = self.downcast::<CSSMatrixComponent>() {
            value.matrix_data(cx)
        } else {
            unreachable!("Every transform component has a concrete type")
        }?;
        if self.is_2d() {
            Ok(Transform3D::new(
                matrix.m11, matrix.m12, 0.0, 0.0, matrix.m21, matrix.m22, 0.0, 0.0, 0.0, 0.0, 1.0,
                0.0, matrix.m41, matrix.m42, 0.0, 1.0,
            ))
        } else {
            Ok(matrix)
        }
    }
}

impl CSSTransformComponentMethods<crate::DomTypeHolder> for CSSTransformComponent {
    fn Stringifier(&self) -> DOMString {
        self.serialize().into()
    }

    fn Is2D(&self) -> bool {
        self.is_2d()
    }

    fn SetIs2D(&self, is_2d: bool) {
        if self.downcast::<CSSSkew>().is_some() ||
            self.downcast::<CSSSkewX>().is_some() ||
            self.downcast::<CSSSkewY>().is_some() ||
            self.downcast::<CSSPerspective>().is_some()
        {
            return;
        }
        self.is_2d.set(is_2d);
    }

    fn ToMatrix(&self, cx: &mut JSContext) -> Fallible<DomRoot<DOMMatrix>> {
        let matrix = self.matrix_data(cx)?;
        Ok(DOMMatrix::new(cx, &self.global(), self.is_2d(), matrix))
    }
}
