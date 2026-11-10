/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::num::Finite;
use script_bindings::reflector::{reflect_dom_object, reflect_dom_object_with_proto};
use style::typed_om::numeric_type::NumericType;

use super::cssnumericvalue::CSSNumericValue;
use crate::dom::bindings::codegen::Bindings::CSSUnitValueBinding::CSSUnitValueMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::{DOMString, USVString};
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSUnitValue {
    css_numeric_value: CSSNumericValue,
    value: Cell<f64>,
    unit: String,
}

impl CSSUnitValue {
    fn new_inherited(value: f64, unit: String) -> Self {
        Self {
            css_numeric_value: CSSNumericValue::new_inherited(),
            value: Cell::new(value),
            unit,
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        value: f64,
        unit: &str,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self::new_inherited(value, unit.to_owned())),
            global,
        )
    }

    pub(crate) fn serialize(&self) -> String {
        let suffix = match self.unit.as_str() {
            "number" => "",
            "percent" => "%",
            unit => unit,
        };
        format!("{}{}", self.value.get(), suffix)
    }
}

impl CSSUnitValueMethods<crate::DomTypeHolder> for CSSUnitValue {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssunitvalue-cssunitvalue>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        value: Finite<f64>,
        unit: USVString,
    ) -> Fallible<DomRoot<Self>> {
        let unit = unit.to_string().to_ascii_lowercase();
        NumericType::try_from_unit(&unit)
            .map_err(|_| Error::Type(c"Unknown CSS unit".to_owned()))?;
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self::new_inherited(*value, unit)),
            global,
            proto,
        ))
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssunitvalue-value>
    fn Value(&self) -> Finite<f64> {
        Finite::wrap(self.value.get())
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssunitvalue-value>
    fn SetValue(&self, value: Finite<f64>) {
        self.value.set(*value);
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssunitvalue-unit>
    fn Unit(&self) -> DOMString {
        DOMString::from(self.unit.clone())
    }
}
