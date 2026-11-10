/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;
use std::collections::HashSet;

use cssparser::{Parser, serialize_identifier};
use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};
use servo_url::ServoUrl;

use crate::dom::bindings::codegen::Bindings::CSSBinding::CSSMethods;
use crate::dom::bindings::codegen::Bindings::CSSStyleValueBinding::CSSStyleValueMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::{DOMString, USVString};
use crate::dom::css::css::CSS;
use crate::dom::css::csskeywordvalue::CSSKeywordValue;
use crate::dom::css::cssunparsedvalue::CSSUnparsedValue;
use crate::dom::css::stylepropertymapreadonly::StylePropertyMapReadOnly;
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSStyleValue {
    reflector: Reflector,
    value: RefCell<String>,
    associated_property: Option<String>,
}

impl CSSStyleValue {
    pub(crate) fn new_inherited(value: String) -> CSSStyleValue {
        CSSStyleValue {
            reflector: Reflector::new(),
            value: RefCell::new(value),
            associated_property: None,
        }
    }

    pub(crate) fn value(&self) -> String {
        self.value.borrow().clone()
    }

    pub(crate) fn set_value(&self, value: String) {
        *self.value.borrow_mut() = value;
    }

    pub(crate) fn associated_property(&self) -> Option<&str> {
        self.associated_property.as_deref()
    }

    pub(crate) fn new_for_property(
        cx: &mut JSContext,
        global: &GlobalScope,
        value: String,
        property: String,
    ) -> DomRoot<CSSStyleValue> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector: Reflector::new(),
                value: RefCell::new(value),
                associated_property: Some(property),
            }),
            global,
        )
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        value: String,
    ) -> DomRoot<CSSStyleValue> {
        reflect_dom_object(cx, Box::new(CSSStyleValue::new_inherited(value)), global)
    }

    fn list_valued_property(property: &str) -> bool {
        matches!(
            property,
            "transition-property" |
                "transition-duration" |
                "transition-timing-function" |
                "transition-delay" |
                "transition-behavior" |
                "animation-name" |
                "animation-duration" |
                "animation-timing-function" |
                "animation-delay" |
                "animation-iteration-count" |
                "animation-direction" |
                "animation-fill-mode" |
                "animation-play-state" |
                "animation-composition" |
                "animation-timeline" |
                "background-image" |
                "background-position-x" |
                "background-position-y" |
                "background-size" |
                "background-repeat" |
                "background-attachment" |
                "background-origin" |
                "background-clip" |
                "mask-image" |
                "mask-position" |
                "mask-size" |
                "mask-repeat"
        )
    }

    fn parse_values(
        cx: &mut JSContext,
        global: &GlobalScope,
        property: USVString,
        css_text: USVString,
        multiple: bool,
    ) -> Fallible<Vec<DomRoot<CSSStyleValue>>> {
        let property =
            StylePropertyMapReadOnly::validated_property(DOMString::from(property.to_string()))?;
        let css_text = css_text.to_string();
        if css_text.trim().is_empty() ||
            !CSS::Supports(
                global.as_window(),
                property.clone(),
                DOMString::from(css_text.clone()),
            )
        {
            return Err(Error::Type(c"Invalid CSS property value".to_owned()));
        }

        let mut values = if Self::list_valued_property(&property.str()) {
            let mut parser = Parser::new(&css_text);
            parser
                .parse_comma_separated(|input| {
                    let start = input.position();
                    while input.next_including_whitespace_and_comments().is_ok() {}
                    Ok::<_, cssparser::ParseError<()>>(input.slice_from(start).trim().to_owned())
                })
                .map_err(|_| Error::Type(c"Invalid CSS property value".to_owned()))?
        } else {
            vec![css_text]
        };

        if !multiple {
            values.truncate(1);
        }
        Ok(values
            .into_iter()
            .map(|value| {
                if property.str().starts_with("--") || value.contains("var(") {
                    return DomRoot::upcast(CSSUnparsedValue::from_string(cx, global, value));
                }
                let keyword = {
                    let mut parser = Parser::new(&value);
                    let ident = parser.expect_ident().ok().map(ToString::to_string);
                    ident.filter(|_| parser.expect_exhausted().is_ok())
                };
                if let Some(keyword) = keyword {
                    DomRoot::upcast(CSSKeywordValue::new(cx, global, keyword))
                } else {
                    Self::new_for_property(cx, global, value, property.to_string())
                }
            })
            .collect())
    }
}

impl CSSStyleValueMethods<crate::DomTypeHolder> for CSSStyleValue {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssstylevalue-parse>
    fn Parse(
        cx: &mut JSContext,
        global: &GlobalScope,
        property: USVString,
        css_text: USVString,
    ) -> Fallible<DomRoot<CSSStyleValue>> {
        Ok(Self::parse_values(cx, global, property, css_text, false)?
            .into_iter()
            .next()
            .unwrap())
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssstylevalue-parseall>
    fn ParseAll(
        cx: &mut JSContext,
        global: &GlobalScope,
        property: USVString,
        css_text: USVString,
    ) -> Fallible<Vec<DomRoot<CSSStyleValue>>> {
        Self::parse_values(cx, global, property, css_text, true)
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#CSSStyleValue-stringification-behavior>
    fn Stringifier(&self) -> DOMString {
        if let Some(unparsed) = self.downcast::<CSSUnparsedValue>() {
            return DOMString::from(unparsed.serialize(&mut HashSet::new()));
        }
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
