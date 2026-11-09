/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::LocalName;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};
use script_bindings::str::DOMString;

use crate::dom::bindings::codegen::Bindings::SVGStringListBinding::SVGStringListMethods;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::node::NodeTraits;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::is_svg_whitespace;

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGStringList>
///
/// Reflects an attribute whose value is a list of strings, separated by whitespace or, for
/// `systemLanguage`, by commas. Its items are strings, not objects, so it does not need to
/// keep them: they are parsed from the attribute whenever they are used.
#[dom_struct]
pub(crate) struct SVGStringList {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    /// Whether the items are separated by commas rather than whitespace.
    comma_separated: bool,
}

impl SVGStringList {
    pub(crate) fn new(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: LocalName,
        comma_separated: bool,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                element: Dom::from_ref(element),
                attribute,
                comma_separated,
            }),
            &*element.owner_window(),
        )
    }

    fn items(&self) -> Vec<String> {
        let Some(value) = self.element.svg_attribute(&self.attribute) else {
            return Vec::new();
        };
        if self.comma_separated {
            value
                .split(',')
                .map(|item| item.trim_matches(is_svg_whitespace))
                .map(str::to_owned)
                .collect()
        } else {
            value
                .split(is_svg_whitespace)
                .filter(|item| !item.is_empty())
                .map(str::to_owned)
                .collect()
        }
    }

    fn set_items(&self, cx: &mut JSContext, items: Vec<String>) {
        let separator = if self.comma_separated { ", " } else { " " };
        self.element
            .set_svg_attribute(cx, &self.attribute, items.join(separator));
    }

    fn index_error() -> Error {
        Error::IndexSize(Some("The index is out of range".into()))
    }
}

impl SVGStringListMethods<crate::DomTypeHolder> for SVGStringList {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__length>
    fn Length(&self) -> u32 {
        self.items().len() as u32
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__numberOfItems>
    fn NumberOfItems(&self) -> u32 {
        self.items().len() as u32
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__clear>
    fn Clear(&self, cx: &mut JSContext) {
        self.set_items(cx, Vec::new());
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__initialize>
    fn Initialize(&self, cx: &mut JSContext, new_item: DOMString) -> DOMString {
        self.set_items(cx, vec![String::from(new_item.str().as_ref())]);
        new_item
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__getItem>
    fn GetItem(&self, index: u32) -> Fallible<DOMString> {
        self.items()
            .into_iter()
            .nth(index as usize)
            .map(DOMString::from)
            .ok_or_else(Self::index_error)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__getItem>
    fn IndexedGetter(&self, index: u32) -> Fallible<Option<DOMString>> {
        Ok(self.GetItem(index).ok())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__insertItemBefore>
    fn InsertItemBefore(&self, cx: &mut JSContext, new_item: DOMString, index: u32) -> DOMString {
        let mut items = self.items();
        let index = (index as usize).min(items.len());
        items.insert(index, String::from(new_item.str().as_ref()));
        self.set_items(cx, items);
        new_item
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__replaceItem>
    fn ReplaceItem(
        &self,
        cx: &mut JSContext,
        new_item: DOMString,
        index: u32,
    ) -> Fallible<DOMString> {
        let mut items = self.items();
        let item = items
            .get_mut(index as usize)
            .ok_or_else(Self::index_error)?;
        *item = String::from(new_item.str().as_ref());
        self.set_items(cx, items);
        Ok(new_item)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__removeItem>
    fn RemoveItem(&self, cx: &mut JSContext, index: u32) -> Fallible<DOMString> {
        let mut items = self.items();
        if index as usize >= items.len() {
            return Err(Self::index_error());
        }
        let item = items.remove(index as usize);
        self.set_items(cx, items);
        Ok(DOMString::from(item))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__appendItem>
    fn AppendItem(&self, cx: &mut JSContext, new_item: DOMString) -> DOMString {
        let mut items = self.items();
        items.push(String::from(new_item.str().as_ref()));
        self.set_items(cx, items);
        new_item
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__setter>
    fn IndexedSetter(&self, cx: &mut JSContext, index: u32, new_item: DOMString) -> ErrorResult {
        self.ReplaceItem(cx, new_item, index).map(|_| ())
    }
}
