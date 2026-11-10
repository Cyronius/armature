/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::codegen::GenericBindings::CSSMathValueBinding::CSSMathOperator;
use style::typed_om::numeric_type::NumericType;

use super::cssmathclamp::CSSMathClamp;
use super::cssmathinvert::CSSMathInvert;
use super::cssmathmax::CSSMathMax;
use super::cssmathmin::CSSMathMin;
use super::cssmathnegate::CSSMathNegate;
use super::cssmathproduct::CSSMathProduct;
use super::cssmathsum::CSSMathSum;
use super::cssnumericvalue::CSSNumericValue;
use super::cssunitvalue::CSSUnitValue;
use crate::dom::bindings::codegen::Bindings::CSSMathValueBinding::CSSMathValueMethods;
use crate::dom::bindings::codegen::UnionTypes::DoubleOrCSSNumericValue;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSMathValue {
    css_numeric_value: CSSNumericValue,
    operator: CSSMathOperator,
    #[no_trace]
    #[ignore_malloc_size_of = "Stylo numeric type"]
    numeric_type: NumericType,
}

impl CSSMathValue {
    pub(crate) fn new_inherited(operator: CSSMathOperator, numeric_type: NumericType) -> Self {
        Self {
            css_numeric_value: CSSNumericValue::new_inherited(),
            operator,
            numeric_type,
        }
    }

    pub(crate) fn numeric_type(&self) -> NumericType {
        self.numeric_type.clone()
    }

    pub(crate) fn serialize(&self) -> String {
        if let Some(sum) = self.downcast::<CSSMathSum>() {
            return format!(
                "calc({})",
                sum.values()
                    .iter()
                    .map(|value| value.serialize())
                    .collect::<Vec<_>>()
                    .join(" + ")
            );
        }
        if let Some(product) = self.downcast::<CSSMathProduct>() {
            return format!(
                "calc({})",
                product
                    .values()
                    .iter()
                    .map(|value| value.serialize())
                    .collect::<Vec<_>>()
                    .join(" * ")
            );
        }
        for (name, values) in [
            ("min", self.downcast::<CSSMathMin>().map(CSSMathMin::values)),
            ("max", self.downcast::<CSSMathMax>().map(CSSMathMax::values)),
        ] {
            if let Some(values) = values {
                return format!(
                    "{}({})",
                    name,
                    values
                        .iter()
                        .map(|value| value.serialize())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        if let Some(negate) = self.downcast::<CSSMathNegate>() {
            return format!("calc(-1 * {})", negate.value().serialize());
        }
        if let Some(invert) = self.downcast::<CSSMathInvert>() {
            return format!("calc(1 / {})", invert.value().serialize());
        }
        let values = self
            .downcast::<CSSMathClamp>()
            .expect("Every CSSMathValue has a concrete subtype")
            .values();
        format!(
            "clamp({})",
            values
                .iter()
                .map(|value| value.serialize())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    pub(crate) fn rectify(
        cx: &mut JSContext,
        global: &GlobalScope,
        value: DoubleOrCSSNumericValue,
    ) -> DomRoot<CSSNumericValue> {
        match value {
            DoubleOrCSSNumericValue::Double(number) => {
                DomRoot::upcast(CSSUnitValue::new(cx, global, *number, "number"))
            },
            DoubleOrCSSNumericValue::CSSNumericValue(value) => value,
        }
    }

    pub(crate) fn type_from_values(
        values: &[DomRoot<CSSNumericValue>],
        product: bool,
    ) -> Fallible<NumericType> {
        let types: Vec<_> = values.iter().map(|value| value.numeric_type()).collect();
        let result = if product {
            NumericType::multiply_types(types.iter())
        } else {
            NumericType::add_types(types.iter())
        };
        result.map_err(|_| Error::Type(c"Incompatible CSS numeric types".to_owned()))
    }
}

impl CSSMathValueMethods<crate::DomTypeHolder> for CSSMathValue {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssmathvalue-operator>
    fn Operator(&self) -> CSSMathOperator {
        self.operator
    }
}
