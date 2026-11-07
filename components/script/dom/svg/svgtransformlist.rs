/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::LocalName;
use js::context::JSContext;
use script_bindings::cell::DomRefCell;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::DOMMatrixBinding::DOMMatrix2DInit;
use crate::dom::bindings::codegen::Bindings::SVGTransformListBinding::SVGTransformListMethods;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::dommatrixreadonly::dommatrix2dinit_to_matrix;
use crate::dom::node::NodeTraits;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtransform::{SVGTransform, SVGTransformValue};
use crate::dom::svg::svgtypes::{is_svg_whitespace, parse_number_prefix};

/// <https://svgwg.org/svg2-draft/coords.html#InterfaceSVGTransformList>
///
/// The base or animated value of an `SVGAnimatedTransformList`, which reflects an attribute.
/// The list is synchronized with the attribute whenever it or one of its numbers is used
/// after the attribute changed.
#[dom_struct]
pub(crate) struct SVGTransformList {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    read_only: bool,
    items: DomRefCell<Vec<Dom<SVGTransform>>>,
    /// The value of the attribute that the items were last synchronized with.
    synchronized_value: DomRefCell<Option<Option<String>>>,
}

/// Parse a transform list. An invalid list is an empty list.
fn parse_transform_value_list(value: Option<&str>) -> Vec<SVGTransformValue> {
    value.and_then(parse_transform_items).unwrap_or_default()
}

/// Parse the items of a transform list, the value of a `transform` attribute.
///
/// <https://drafts.csswg.org/css-transforms/#svg-syntax>
pub(crate) fn parse_transform_items(value: &str) -> Option<Vec<SVGTransformValue>> {
    let mut items = Vec::new();
    let mut rest = value.trim_matches(is_svg_whitespace);
    while !rest.is_empty() {
        let name_end = rest.find(|c: char| !c.is_ascii_alphabetic())?;
        let name = &rest[..name_end];
        rest = rest[name_end..].trim_start_matches(is_svg_whitespace);
        rest = rest.strip_prefix('(')?;
        let close = rest.find(')')?;
        let arguments = parse_arguments(&rest[..close])?;
        rest = rest[close + 1..].trim_start_matches(is_svg_whitespace);
        if let Some(after_comma) = rest.strip_prefix(',') {
            rest = after_comma.trim_start_matches(is_svg_whitespace);
            if rest.is_empty() {
                return None;
            }
        }
        items.push(match (name, arguments.as_slice()) {
            ("matrix", [a, b, c, d, e, f]) => {
                SVGTransformValue::matrix(kurbo::Affine::new([*a, *b, *c, *d, *e, *f]))
            },
            ("translate", [x]) => SVGTransformValue::translate(*x, 0.0),
            ("translate", [x, y]) => SVGTransformValue::translate(*x, *y),
            ("scale", [x]) => SVGTransformValue::scale(*x, *x),
            ("scale", [x, y]) => SVGTransformValue::scale(*x, *y),
            ("rotate", [angle]) => SVGTransformValue::rotate(*angle, 0.0, 0.0),
            ("rotate", [angle, x, y]) => SVGTransformValue::rotate(*angle, *x, *y),
            ("skewX", [angle]) => SVGTransformValue::skew_x(*angle),
            ("skewY", [angle]) => SVGTransformValue::skew_y(*angle),
            _ => return None,
        });
    }
    Some(items)
}

/// Parse the arguments of a transform function: numbers separated by whitespace and
/// optionally a comma.
fn parse_arguments(value: &str) -> Option<Vec<f64>> {
    let mut numbers = Vec::new();
    let mut rest = value.trim_matches(is_svg_whitespace);
    while !rest.is_empty() {
        let (number, after) = parse_number_prefix(rest)?;
        numbers.push(number as f64);
        rest = after.trim_start_matches(is_svg_whitespace);
        if let Some(after_comma) = rest.strip_prefix(',') {
            rest = after_comma.trim_start_matches(is_svg_whitespace);
            if rest.is_empty() {
                return None;
            }
        }
    }
    Some(numbers)
}

impl SVGTransformList {
    pub(crate) fn new(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: LocalName,
        read_only: bool,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                element: Dom::from_ref(element),
                attribute,
                read_only,
                items: Default::default(),
                synchronized_value: Default::default(),
            }),
            &*element.owner_window(),
        )
    }

    /// <https://svgwg.org/svg2-draft/types.html#TermSynchronizeList>
    pub(crate) fn synchronize(&self, cx: &mut JSContext) {
        let value = self.element.svg_attribute(&self.attribute);
        if self.synchronized_value.borrow().as_ref() == Some(&value) {
            return;
        }
        let values = parse_transform_value_list(value.as_deref());
        *self.synchronized_value.borrow_mut() = Some(value);

        // > If the new value has fewer items than the list, detach the items beyond its
        // > length and remove them.
        let existing = {
            let mut items = self.items.borrow_mut();
            if values.len() < items.len() {
                let removed: Vec<_> = items
                    .drain(values.len()..)
                    .map(|item| item.as_rooted())
                    .collect();
                for item in removed {
                    item.detach();
                }
            }
            for (item, value) in items.iter().zip(values.iter()) {
                item.set_raw_value(*value);
            }
            items.len()
        };
        // > Append new items for the rest of the values.
        for value in values.into_iter().skip(existing) {
            let item = SVGTransform::new(cx, &self.global(), value, Some(self), self.read_only);
            self.items.borrow_mut().push(Dom::from_ref(&*item));
        }
    }

    /// Serialize the items into the attribute.
    pub(crate) fn commit(&self, cx: &mut JSContext) {
        let value = self
            .items
            .borrow()
            .iter()
            .map(|item| item.value().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        *self.synchronized_value.borrow_mut() = Some(Some(value.clone()));
        self.element.set_svg_attribute(cx, &self.attribute, value);
    }

    fn check_writable(&self) -> ErrorResult {
        if self.read_only {
            return Err(Error::NoModificationAllowed(Some(
                "This SVGTransformList is read-only".into(),
            )));
        }
        Ok(())
    }

    /// The item to insert for `new_item`: `new_item` itself, or a copy when it already
    /// belongs to a list.
    fn item_for_insertion(
        &self,
        cx: &mut JSContext,
        new_item: &SVGTransform,
    ) -> DomRoot<SVGTransform> {
        if new_item.is_attached() {
            return SVGTransform::new(
                cx,
                &self.global(),
                new_item.value(),
                Some(self),
                self.read_only,
            );
        }
        new_item.attach(self, self.read_only);
        DomRoot::from_ref(new_item)
    }

    fn index_error() -> Error {
        Error::IndexSize(Some("The index is out of range".into()))
    }
}

impl SVGTransformListMethods<crate::DomTypeHolder> for SVGTransformList {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__length>
    fn Length(&self, cx: &mut JSContext) -> u32 {
        self.NumberOfItems(cx)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__numberOfItems>
    fn NumberOfItems(&self, cx: &mut JSContext) -> u32 {
        self.synchronize(cx);
        self.items.borrow().len() as u32
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__clear>
    fn Clear(&self, cx: &mut JSContext) -> ErrorResult {
        self.check_writable()?;
        self.synchronize(cx);
        let removed: Vec<_> = self
            .items
            .borrow_mut()
            .drain(..)
            .map(|item| item.as_rooted())
            .collect();
        for item in removed {
            item.detach();
        }
        self.commit(cx);
        Ok(())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__initialize>
    fn Initialize(
        &self,
        cx: &mut JSContext,
        new_item: &SVGTransform,
    ) -> Fallible<DomRoot<SVGTransform>> {
        self.check_writable()?;
        self.synchronize(cx);
        let removed: Vec<_> = self
            .items
            .borrow_mut()
            .drain(..)
            .map(|item| item.as_rooted())
            .collect();
        for item in removed {
            item.detach();
        }
        let item = self.item_for_insertion(cx, new_item);
        self.items.borrow_mut().push(Dom::from_ref(&*item));
        self.commit(cx);
        Ok(item)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__getItem>
    fn GetItem(&self, cx: &mut JSContext, index: u32) -> Fallible<DomRoot<SVGTransform>> {
        self.synchronize(cx);
        self.items
            .borrow()
            .get(index as usize)
            .map(|item| item.as_rooted())
            .ok_or_else(Self::index_error)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__getItem>
    fn IndexedGetter(
        &self,
        cx: &mut JSContext,
        index: u32,
    ) -> Fallible<Option<DomRoot<SVGTransform>>> {
        Ok(self.GetItem(cx, index).ok())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__insertItemBefore>
    fn InsertItemBefore(
        &self,
        cx: &mut JSContext,
        new_item: &SVGTransform,
        index: u32,
    ) -> Fallible<DomRoot<SVGTransform>> {
        self.check_writable()?;
        self.synchronize(cx);
        let item = self.item_for_insertion(cx, new_item);
        {
            let mut items = self.items.borrow_mut();
            let index = (index as usize).min(items.len());
            items.insert(index, Dom::from_ref(&*item));
        }
        self.commit(cx);
        Ok(item)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__replaceItem>
    fn ReplaceItem(
        &self,
        cx: &mut JSContext,
        new_item: &SVGTransform,
        index: u32,
    ) -> Fallible<DomRoot<SVGTransform>> {
        self.check_writable()?;
        self.synchronize(cx);
        if index as usize >= self.items.borrow().len() {
            return Err(Self::index_error());
        }
        let item = self.item_for_insertion(cx, new_item);
        let old = self.items.borrow()[index as usize].as_rooted();
        self.items.borrow_mut()[index as usize] = Dom::from_ref(&*item);
        if *old != *item {
            old.detach();
        }
        self.commit(cx);
        Ok(item)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__removeItem>
    fn RemoveItem(&self, cx: &mut JSContext, index: u32) -> Fallible<DomRoot<SVGTransform>> {
        self.check_writable()?;
        self.synchronize(cx);
        if index as usize >= self.items.borrow().len() {
            return Err(Self::index_error());
        }
        let item = self.items.borrow_mut().remove(index as usize).as_rooted();
        item.detach();
        self.commit(cx);
        Ok(item)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__appendItem>
    fn AppendItem(
        &self,
        cx: &mut JSContext,
        new_item: &SVGTransform,
    ) -> Fallible<DomRoot<SVGTransform>> {
        self.check_writable()?;
        self.synchronize(cx);
        let item = self.item_for_insertion(cx, new_item);
        self.items.borrow_mut().push(Dom::from_ref(&*item));
        self.commit(cx);
        Ok(item)
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransformList__createSVGTransformFromMatrix>
    fn CreateSVGTransformFromMatrix(
        &self,
        cx: &mut JSContext,
        matrix: &DOMMatrix2DInit,
    ) -> Fallible<DomRoot<SVGTransform>> {
        let matrix = dommatrix2dinit_to_matrix(matrix)?;
        Ok(SVGTransform::new(
            cx,
            &self.global(),
            SVGTransformValue::matrix(kurbo::Affine::new([
                matrix.m11, matrix.m12, matrix.m21, matrix.m22, matrix.m31, matrix.m32,
            ])),
            None,
            false,
        ))
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransformList__consolidate>
    fn Consolidate(&self, cx: &mut JSContext) -> Fallible<Option<DomRoot<SVGTransform>>> {
        self.check_writable()?;
        self.synchronize(cx);
        if self.items.borrow().is_empty() {
            return Ok(None);
        }
        let matrix = self
            .items
            .borrow()
            .iter()
            .fold(kurbo::Affine::IDENTITY, |matrix, item| {
                matrix * item.value().matrix
            });
        let removed: Vec<_> = self
            .items
            .borrow_mut()
            .drain(..)
            .map(|item| item.as_rooted())
            .collect();
        for item in removed {
            item.detach();
        }
        let item = SVGTransform::new(
            cx,
            &self.global(),
            SVGTransformValue::matrix(matrix),
            Some(self),
            self.read_only,
        );
        self.items.borrow_mut().push(Dom::from_ref(&*item));
        self.commit(cx);
        Ok(Some(item))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__setter>
    fn IndexedSetter(
        &self,
        cx: &mut JSContext,
        index: u32,
        new_item: &SVGTransform,
    ) -> ErrorResult {
        self.ReplaceItem(cx, new_item, index).map(|_| ())
    }
}
