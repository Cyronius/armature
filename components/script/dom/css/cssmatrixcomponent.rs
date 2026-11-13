/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use euclid::default::Transform3D;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::reflector::reflect_dom_object_with_proto;

use super::csstransformcomponent::CSSTransformComponent;
use crate::dom::bindings::codegen::Bindings::CSSMatrixComponentBinding::{
    CSSMatrixComponentMethods, CSSMatrixComponentOptions,
};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{DomRoot, MutDom};
use crate::dom::dommatrix::DOMMatrix;
use crate::dom::dommatrixreadonly::DOMMatrixReadOnly;
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSMatrixComponent {
    css_transform_component: CSSTransformComponent,
    matrix: MutDom<DOMMatrix>,
}

impl CSSMatrixComponent {
    pub(crate) fn serialize(&self) -> String {
        let matrix = self.matrix.get();
        let data = matrix.upcast::<DOMMatrixReadOnly>().matrix();
        if self.css_transform_component.is_2d() {
            format!(
                "matrix({}, {}, {}, {}, {}, {})",
                data.m11, data.m12, data.m21, data.m22, data.m41, data.m42
            )
        } else {
            format!(
                "matrix3d({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                data.m11,
                data.m12,
                data.m13,
                data.m14,
                data.m21,
                data.m22,
                data.m23,
                data.m24,
                data.m31,
                data.m32,
                data.m33,
                data.m34,
                data.m41,
                data.m42,
                data.m43,
                data.m44
            )
        }
    }

    pub(crate) fn matrix_data(
        &self,
        _cx: &mut JSContext,
    ) -> crate::dom::bindings::error::Fallible<Transform3D<f64>> {
        Ok(*self.matrix.get().upcast::<DOMMatrixReadOnly>().matrix())
    }
}

impl CSSMatrixComponentMethods<crate::DomTypeHolder> for CSSMatrixComponent {
    fn Constructor(
        cx: &mut JSContext,
        global: &crate::dom::window::Window,
        proto: Option<HandleObject>,
        matrix: &DOMMatrixReadOnly,
        options: &CSSMatrixComponentOptions,
    ) -> crate::dom::bindings::error::Fallible<DomRoot<Self>> {
        let global = global.upcast::<GlobalScope>();
        let is_2d = options.is2D.unwrap_or(matrix.is2D());
        let matrix = DOMMatrix::from_readonly(global, matrix, cx);
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                css_transform_component: CSSTransformComponent::new_inherited(is_2d),
                matrix: MutDom::new(&matrix),
            }),
            global,
            proto,
        ))
    }

    fn Matrix(&self) -> DomRoot<DOMMatrix> {
        self.matrix.get()
    }
    fn SetMatrix(&self, value: &DOMMatrix) {
        self.matrix.set(value);
    }
}
