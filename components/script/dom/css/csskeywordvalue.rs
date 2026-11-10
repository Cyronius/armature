/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::reflector::{reflect_dom_object, reflect_dom_object_with_proto};

use super::cssstylevalue::CSSStyleValue;
use crate::dom::bindings::codegen::Bindings::CSSKeywordValueBinding::CSSKeywordValueMethods;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSKeywordValue {
    css_style_value: CSSStyleValue,
}

impl CSSKeywordValue {
    fn new_inherited(value: String) -> Self {
        Self {
            css_style_value: CSSStyleValue::new_inherited(value),
        }
    }

    pub(crate) fn new(cx: &mut JSContext, global: &GlobalScope, value: String) -> DomRoot<Self> {
        reflect_dom_object(cx, Box::new(Self::new_inherited(value)), global)
    }
}

impl CSSKeywordValueMethods<crate::DomTypeHolder> for CSSKeywordValue {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-csskeywordvalue-csskeywordvalue>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        value: DOMString,
    ) -> Fallible<DomRoot<Self>> {
        if value.is_empty() {
            return Err(Error::Type(c"CSSKeywordValue cannot be empty".to_owned()));
        }
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(Self::new_inherited(value.into())),
            global,
            proto,
        ))
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-csskeywordvalue-value>
    fn Value(&self) -> DOMString {
        DOMString::from(self.css_style_value.value())
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-csskeywordvalue-value>
    fn SetValue(&self, value: DOMString) -> ErrorResult {
        if value.is_empty() {
            return Err(Error::Type(c"CSSKeywordValue cannot be empty".to_owned()));
        }
        self.css_style_value.set_value(value.into());
        Ok(())
    }
}
