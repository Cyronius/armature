/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cmp::Ordering;
use std::iter::Iterator;

use cssparser::Parser;
use dom_struct::dom_struct;
use js::context::JSContext;
use js::conversions::ToJSValConvertible;
use js::jsval::UndefinedValue;
use js::rust::MutableHandleValue;
use rustc_hash::FxBuildHasher;
use script_bindings::domstring::TracedDOMString;
use script_bindings::reflector::{Reflector, reflect_dom_object};
use style::custom_properties;
use style::properties::PropertyId;
use stylo_atoms::Atom;

use super::csskeywordvalue::CSSKeywordValue;
use super::cssstyledeclaration::CSSStyleDeclaration;
use super::cssstylevalue::CSSStyleValue;
use crate::dom::bindings::codegen::Bindings::CSSStyleDeclarationBinding::CSSStyleDeclarationMethods;
use crate::dom::bindings::codegen::Bindings::StylePropertyMapReadOnlyBinding::StylePropertyMapReadOnlyMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::bindings::str::DOMString;
use crate::dom::bindings::trace::HashMapTracedValues;
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct StylePropertyMapReadOnly {
    reflector: Reflector,
    entries: HashMapTracedValues<Atom, Dom<CSSStyleValue>, FxBuildHasher>,
    style: Option<Dom<CSSStyleDeclaration>>,
}

impl StylePropertyMapReadOnly {
    pub(crate) fn validated_property(property: DOMString) -> Fallible<DOMString> {
        let name = property.to_string();
        if PropertyId::parse_enabled_for_all_content(&name).is_err() {
            return Err(Error::Type(c"Unrecognized CSS property".to_owned()));
        }
        if name.starts_with("--") {
            Ok(property)
        } else {
            Ok(DOMString::from(name.to_ascii_lowercase()))
        }
    }

    fn new_inherited<Entries>(entries: Entries) -> StylePropertyMapReadOnly
    where
        Entries: IntoIterator<Item = (Atom, Dom<CSSStyleValue>)>,
    {
        StylePropertyMapReadOnly {
            reflector: Reflector::new(),
            entries: HashMapTracedValues(entries.into_iter().collect()),
            style: None,
        }
    }

    pub(crate) fn for_style(style: &CSSStyleDeclaration) -> Self {
        Self {
            reflector: Reflector::new(),
            entries: HashMapTracedValues(Default::default()),
            style: Some(Dom::from_ref(style)),
        }
    }

    pub(crate) fn style(&self) -> Option<&CSSStyleDeclaration> {
        self.style.as_deref()
    }

    fn get_value(&self, cx: &mut JSContext, property: DOMString) -> Option<DomRoot<CSSStyleValue>> {
        if let Some(style) = self.style() {
            let value = style.GetPropertyValue(property).to_string();
            if value.is_empty() {
                return None;
            }
            let global = self.global();
            let is_keyword = {
                let mut parser = Parser::new(&value);
                parser.expect_ident().is_ok() && parser.expect_exhausted().is_ok()
            };
            if is_keyword {
                return Some(DomRoot::upcast(CSSKeywordValue::new(
                    cx,
                    &global,
                    value.into(),
                )));
            }
            return Some(CSSStyleValue::new(cx, &global, value.into()));
        }
        self.entries
            .get(&Atom::from(property))
            .map(|value| DomRoot::from_ref(&**value))
    }

    pub(crate) fn from_iter<Entries>(
        cx: &mut js::context::JSContext,
        global: &GlobalScope,
        entries: Entries,
    ) -> DomRoot<StylePropertyMapReadOnly>
    where
        Entries: IntoIterator<Item = (Atom, String)>,
    {
        let mut keys = Vec::new();
        rooted_vec!(let mut values);
        let iter = entries.into_iter();
        let (lo, _) = iter.size_hint();
        keys.reserve(lo);
        values.reserve(lo);
        for (key, value) in iter {
            let value = CSSStyleValue::new(cx, global, value);
            keys.push(key);
            values.push(Dom::from_ref(&*value));
        }
        let iter = keys.into_iter().zip(values.iter().cloned());
        reflect_dom_object(
            cx,
            Box::new(StylePropertyMapReadOnly::new_inherited(iter)),
            global,
        )
    }
}

impl StylePropertyMapReadOnlyMethods<crate::DomTypeHolder> for StylePropertyMapReadOnly {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-stylepropertymapreadonly-get>
    fn Get(
        &self,
        cx: &mut JSContext,
        property: DOMString,
        mut retval: MutableHandleValue,
    ) -> Fallible<()> {
        let property = Self::validated_property(property)?;
        if let Some(value) = self.get_value(cx, property) {
            value.to_jsval(cx, retval);
        } else {
            retval.set(UndefinedValue());
        }
        Ok(())
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-stylepropertymapreadonly-has>
    fn Has(&self, property: DOMString) -> Fallible<bool> {
        let property = Self::validated_property(property)?;
        if let Some(style) = self.style() {
            return Ok(!style.GetPropertyValue(property).is_empty());
        }
        Ok(self.entries.contains_key(&Atom::from(property)))
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-stylepropertymapreadonly-getproperties>
    fn GetProperties(&self) -> Vec<DOMString> {
        let mut result: Vec<DOMString> = if let Some(style) = self.style() {
            (0..style.Length()).map(|index| style.Item(index)).collect()
        } else {
            self.entries
                .0
                .keys()
                .map(|key| TracedDOMString::from(key.clone()).root())
                .collect()
        };
        // https://drafts.css-houdini.org/css-typed-om-1/#dom-stylepropertymap-getproperties
        // requires this sort order
        result.sort_by(|key1, key2| {
            if let Ok(key1) = custom_properties::parse_name(&key1.str()) {
                if let Ok(key2) = custom_properties::parse_name(&key2.str()) {
                    key1.cmp(key2)
                } else {
                    Ordering::Greater
                }
            } else if custom_properties::parse_name(&key2.str()).is_ok() {
                Ordering::Less
            } else {
                key1.cmp(key2)
            }
        });
        result
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-stylepropertymapreadonly-getall>
    fn GetAll(
        &self,
        cx: &mut JSContext,
        property: DOMString,
    ) -> Fallible<Vec<DomRoot<CSSStyleValue>>> {
        let property = Self::validated_property(property)?;
        Ok(self.get_value(cx, property).into_iter().collect())
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-stylepropertymapreadonly-size>
    fn Size(&self) -> u32 {
        if let Some(style) = self.style() {
            style.Length()
        } else {
            self.entries.0.len() as u32
        }
    }
}
