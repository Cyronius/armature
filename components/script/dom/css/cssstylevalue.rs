/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;

use cssparser::{Parser, serialize_identifier};
use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};
use servo_url::ServoUrl;

use crate::dom::bindings::codegen::Bindings::CSSStyleValueBinding::CSSStyleValueMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::css::csskeywordvalue::CSSKeywordValue;
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSStyleValue {
    reflector: Reflector,
    value: RefCell<String>,
}

impl CSSStyleValue {
    pub(crate) fn new_inherited(value: String) -> CSSStyleValue {
        CSSStyleValue {
            reflector: Reflector::new(),
            value: RefCell::new(value),
        }
    }

    pub(crate) fn value(&self) -> String {
        self.value.borrow().clone()
    }

    pub(crate) fn set_value(&self, value: String) {
        *self.value.borrow_mut() = value;
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        value: String,
    ) -> DomRoot<CSSStyleValue> {
        reflect_dom_object(cx, Box::new(CSSStyleValue::new_inherited(value)), global)
    }
}

impl CSSStyleValueMethods<crate::DomTypeHolder> for CSSStyleValue {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#CSSStyleValue-stringification-behavior>
    fn Stringifier(&self) -> DOMString {
        let value = self.value();
        if self.is::<CSSKeywordValue>() {
            let mut escaped = String::new();
            serialize_identifier(&value, &mut escaped).unwrap();
            DOMString::from(escaped)
        } else {
            DOMString::from(value)
        }
    }
}

impl CSSStyleValue {
    /// Parse the value as a `url()`.
    /// TODO: This should really always be an absolute URL, but we currently
    /// return relative URLs for computed values, so we pass in a base.
    /// <https://github.com/servo/servo/issues/17625>
    pub(crate) fn get_url(&self, base_url: ServoUrl) -> Option<ServoUrl> {
        let value = self.value.borrow();
        let mut parser = Parser::new(&value);
        parser
            .expect_url()
            .ok()
            .and_then(|string| base_url.join(&string).ok())
    }
}
