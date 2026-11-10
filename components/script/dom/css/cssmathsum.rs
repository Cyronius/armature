/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::codegen::GenericBindings::CSSMathValueBinding::CSSMathOperator;
use script_bindings::reflector::reflect_dom_object_with_proto;

use super::cssmathvalue::CSSMathValue;
use super::cssnumericarray::CSSNumericArray;
use super::cssnumericvalue::CSSNumericValue;
use crate::dom::bindings::codegen::Bindings::CSSMathSumBinding::CSSMathSumMethods;
use crate::dom::bindings::codegen::UnionTypes::DoubleOrCSSNumericValue;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSMathSum {
    css_math_value: CSSMathValue,
    values: Dom<CSSNumericArray>,
}

impl CSSMathSum {
    pub(crate) fn values(&self) -> Vec<DomRoot<CSSNumericValue>> {
        self.values.values()
    }
}

impl CSSMathSumMethods<crate::DomTypeHolder> for CSSMathSum {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#complex-numeric>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        args: Vec<DoubleOrCSSNumericValue>,
    ) -> Fallible<DomRoot<Self>> {
        if args.is_empty() {
            return Err(Error::Syntax(Some(
                "CSS math values need at least one argument".into(),
            )));
        }
        let values: Vec<_> = args
            .into_iter()
            .map(|arg| CSSMathValue::rectify(cx, global, arg))
            .collect();
        let numeric_type = CSSMathValue::type_from_values(&values, false)?;
        let array = CSSNumericArray::new(cx, global, values);
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                css_math_value: CSSMathValue::new_inherited(CSSMathOperator::Sum, numeric_type),
                values: Dom::from_ref(&array),
            }),
            global,
            proto,
        ))
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssmathsum-values>
    fn Values(&self) -> DomRoot<CSSNumericArray> {
        self.values.as_rooted()
    }
}
