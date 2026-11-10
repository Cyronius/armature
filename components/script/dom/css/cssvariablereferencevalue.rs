/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;
use std::collections::HashSet;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::reflector::{Reflector, reflect_dom_object_with_proto};
use style::properties::PropertyId;

use super::cssunparsedvalue::CSSUnparsedValue;
use crate::dom::bindings::codegen::Bindings::CSSVariableReferenceValueBinding::CSSVariableReferenceValueMethods;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::bindings::str::USVString;
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSVariableReferenceValue {
    reflector: Reflector,
    variable: RefCell<String>,
    fallback: Option<Dom<CSSUnparsedValue>>,
}

impl CSSVariableReferenceValue {
    fn valid_name(name: &str) -> bool {
        matches!(
            PropertyId::parse_enabled_for_all_content(name),
            Ok(PropertyId::Custom(_))
        )
    }

    pub(crate) fn serialize(&self, seen: &mut HashSet<usize>) -> String {
        let mut result = format!("var({}", self.variable.borrow());
        if let Some(fallback) = &self.fallback {
            result.push(',');
            result.push_str(&fallback.serialize(seen));
        }
        result.push(')');
        result
    }
}

impl CSSVariableReferenceValueMethods<crate::DomTypeHolder> for CSSVariableReferenceValue {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssvariablereferencevalue-cssvariablereferencevalue>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        variable: USVString,
        fallback: Option<&CSSUnparsedValue>,
    ) -> Fallible<DomRoot<Self>> {
        if !Self::valid_name(&variable.to_string()) {
            return Err(Error::Type(c"Invalid custom property name".to_owned()));
        }
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self {
                reflector: Reflector::new(),
                variable: RefCell::new(variable.to_string()),
                fallback: fallback.map(Dom::from_ref),
            }),
            global,
            proto,
        ))
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssvariablereferencevalue-variable>
    fn Variable(&self) -> USVString {
        USVString::from(self.variable.borrow().clone())
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssvariablereferencevalue-variable>
    fn SetVariable(&self, variable: USVString) -> ErrorResult {
        if !Self::valid_name(&variable.to_string()) {
            return Err(Error::Type(c"Invalid custom property name".to_owned()));
        }
        *self.variable.borrow_mut() = variable.to_string();
        Ok(())
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssvariablereferencevalue-fallback>
    fn GetFallback(&self) -> Option<DomRoot<CSSUnparsedValue>> {
        self.fallback.as_ref().map(|fallback| fallback.as_rooted())
    }
}
