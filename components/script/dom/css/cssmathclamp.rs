/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::codegen::GenericBindings::CSSMathValueBinding::CSSMathOperator;
use script_bindings::reflector::reflect_dom_object_with_proto;
use style::typed_om::numeric_type::NumericType;

use super::cssmathvalue::CSSMathValue;
use super::cssnumericvalue::CSSNumericValue;
use crate::dom::bindings::codegen::Bindings::CSSMathClampBinding::CSSMathClampMethods;
use crate::dom::bindings::codegen::UnionTypes::DoubleOrCSSNumericValue;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSMathClamp {
    css_math_value: CSSMathValue,
    lower: Dom<CSSNumericValue>,
    value: Dom<CSSNumericValue>,
    upper: Dom<CSSNumericValue>,
}

impl CSSMathClamp {
    pub(crate) fn values(&self) -> [DomRoot<CSSNumericValue>; 3] {
        [
            self.lower.as_rooted(),
            self.value.as_rooted(),
            self.upper.as_rooted(),
        ]
    }
}

impl CSSMathClampMethods<crate::DomTypeHolder> for CSSMathClamp {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#complex-numeric>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        lower: DoubleOrCSSNumericValue,
        value: DoubleOrCSSNumericValue,
        upper: DoubleOrCSSNumericValue,
    ) -> Fallible<DomRoot<Self>> {
        let lower = CSSMathValue::rectify(cx, global, lower);
        let value = CSSMathValue::rectify(cx, global, value);
        let upper = CSSMathValue::rectify(cx, global, upper);
        let types = [
            lower.numeric_type(),
            value.numeric_type(),
            upper.numeric_type(),
        ];
        let numeric_type = NumericType::add_types(types.iter())
            .map_err(|_| Error::Type(c"Incompatible CSS numeric types".to_owned()))?;
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                css_math_value: CSSMathValue::new_inherited(CSSMathOperator::Clamp, numeric_type),
                lower: Dom::from_ref(&lower),
                value: Dom::from_ref(&value),
                upper: Dom::from_ref(&upper),
            }),
            global,
            proto,
        ))
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssmathclamp-lower>
    fn Lower(&self) -> DomRoot<CSSNumericValue> {
        self.lower.as_rooted()
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssmathclamp-value>
    fn Value(&self) -> DomRoot<CSSNumericValue> {
        self.value.as_rooted()
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssmathclamp-upper>
    fn Upper(&self) -> DomRoot<CSSNumericValue> {
        self.upper.as_rooted()
    }
}
