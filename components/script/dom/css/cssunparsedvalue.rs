/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashSet;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::cell::DomRefCell;
use script_bindings::reflector::{reflect_dom_object, reflect_dom_object_with_proto};

use super::cssstylevalue::CSSStyleValue;
use super::cssvariablereferencevalue::CSSVariableReferenceValue;
use crate::dom::bindings::codegen::Bindings::CSSUnparsedValueBinding::CSSUnparsedValueMethods;
use crate::dom::bindings::codegen::UnionTypes::USVStringOrCSSVariableReferenceValue;
use crate::dom::bindings::error::{Error, ErrorResult};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::bindings::str::USVString;
use crate::dom::globalscope::GlobalScope;

#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
#[derive(JSTraceable, MallocSizeOf)]
enum Segment {
    String(String),
    Variable(Dom<CSSVariableReferenceValue>),
}

impl Segment {
    fn from_union(value: USVStringOrCSSVariableReferenceValue) -> Self {
        match value {
            USVStringOrCSSVariableReferenceValue::USVString(value) => {
                Self::String(value.to_string())
            },
            USVStringOrCSSVariableReferenceValue::CSSVariableReferenceValue(value) => {
                Self::Variable(Dom::from_ref(&value))
            },
        }
    }

    fn as_union(&self) -> USVStringOrCSSVariableReferenceValue {
        match self {
            Self::String(value) => {
                USVStringOrCSSVariableReferenceValue::USVString(USVString::from(value.clone()))
            },
            Self::Variable(value) => {
                USVStringOrCSSVariableReferenceValue::CSSVariableReferenceValue(value.as_rooted())
            },
        }
    }
}

#[dom_struct]
pub(crate) struct CSSUnparsedValue {
    css_style_value: CSSStyleValue,
    segments: DomRefCell<Vec<Segment>>,
}

impl CSSUnparsedValue {
    fn normalize_cssom_comments(value: &str) -> String {
        let mut result = String::new();
        let mut position = 0;
        let mut quote = None;
        while position < value.len() {
            let rest = &value[position..];
            let character = rest.chars().next().unwrap();
            if let Some(delimiter) = quote {
                result.push(character);
                position += character.len_utf8();
                if character == '\\' && position < value.len() {
                    let escaped = value[position..].chars().next().unwrap();
                    result.push(escaped);
                    position += escaped.len_utf8();
                } else if character == delimiter {
                    quote = None;
                }
            } else if character == '\'' || character == '"' {
                quote = Some(character);
                result.push(character);
                position += character.len_utf8();
            } else if rest.starts_with("/*") {
                if let Some(end) = rest.find("*/") {
                    result.push_str("/**/");
                    position += end + 2;
                } else {
                    result.push_str(rest);
                    break;
                }
            } else {
                result.push(character);
                position += character.len_utf8();
            }
        }
        result
    }

    fn new_inherited(segments: Vec<Segment>) -> Self {
        Self {
            css_style_value: CSSStyleValue::new_inherited(String::new()),
            segments: DomRefCell::new(segments),
        }
    }

    pub(crate) fn from_string(
        cx: &mut JSContext,
        global: &GlobalScope,
        value: String,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self::new_inherited(vec![Segment::String(value)])),
            global,
        )
    }

    pub(crate) fn from_cssom_string(
        cx: &mut JSContext,
        global: &GlobalScope,
        value: String,
    ) -> DomRoot<Self> {
        Self::from_string(cx, global, Self::normalize_cssom_comments(&value))
    }

    pub(crate) fn serialize(&self, seen: &mut HashSet<usize>) -> String {
        let identity = self as *const _ as usize;
        if !seen.insert(identity) {
            return String::new();
        }
        let mut result = String::new();
        let mut previous_string = false;
        for segment in self.segments.borrow().iter() {
            match segment {
                Segment::String(value) => {
                    if previous_string && !value.is_empty() && !result.is_empty() {
                        result.push_str("/**/");
                    }
                    result.push_str(value);
                    previous_string = true;
                },
                Segment::Variable(value) => {
                    result.push_str(&value.serialize(seen));
                    previous_string = false;
                },
            }
        }
        seen.remove(&identity);
        result
    }
}

impl CSSUnparsedValueMethods<crate::DomTypeHolder> for CSSUnparsedValue {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssunparsedvalue-cssunparsedvalue>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        members: Vec<USVStringOrCSSVariableReferenceValue>,
    ) -> DomRoot<Self> {
        reflect_dom_object_with_proto(
            cx,
            Box::new(Self::new_inherited(
                members.into_iter().map(Segment::from_union).collect(),
            )),
            global,
            proto,
        )
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssunparsedvalue-length>
    fn Length(&self) -> u32 {
        self.segments.borrow().len() as u32
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssunparsedvalue-__getter__-index-index>
    fn IndexedGetter(&self, index: u32) -> Option<USVStringOrCSSVariableReferenceValue> {
        self.segments
            .borrow()
            .get(index as usize)
            .map(Segment::as_union)
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssunparsedvalue-__setter__-index-val-val>
    fn IndexedSetter(
        &self,
        index: u32,
        value: USVStringOrCSSVariableReferenceValue,
    ) -> ErrorResult {
        let mut segments = self.segments.borrow_mut();
        if index as usize > segments.len() {
            return Err(Error::Range(
                c"CSSUnparsedValue index is out of range".to_owned(),
            ));
        }
        if index as usize == segments.len() {
            segments.push(Segment::from_union(value));
        } else {
            segments[index as usize] = Segment::from_union(value);
        }
        Ok(())
    }
}
