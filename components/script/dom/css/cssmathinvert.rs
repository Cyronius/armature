/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::codegen::GenericBindings::CSSMathValueBinding::CSSMathOperator;
use script_bindings::reflector::reflect_dom_object_with_proto;

use super::cssmathvalue::CSSMathValue;
use super::cssnumericvalue::CSSNumericValue;
use crate::dom::bindings::codegen::Bindings::CSSMathInvertBinding::CSSMathInvertMethods;
use crate::dom::bindings::codegen::UnionTypes::DoubleOrCSSNumericValue;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSMathInvert {
    css_math_value: CSSMathValue,
    value: Dom<CSSNumericValue>,
}

impl CSSMathInvert {
    pub(crate) fn value(&self) -> DomRoot<CSSNumericValue> {
        self.value.as_rooted()
    }
}

impl CSSMathInvertMethods<crate::DomTypeHolder> for CSSMathInvert {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#complex-numeric>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        arg: DoubleOrCSSNumericValue,
    ) -> DomRoot<Self> {
        let value = CSSMathValue::rectify(cx, global, arg);
        let mut numeric_type = value.numeric_type();
        numeric_type.invert();
        reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                css_math_value: CSSMathValue::new_inherited(CSSMathOperator::Invert, numeric_type),
                value: Dom::from_ref(&value),
            }),
            global,
            proto,
        )
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssmathinvert-value>
    fn Value(&self) -> DomRoot<CSSNumericValue> {
        self.value()
    }
}
