/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, local_name};
use js::context::JSContext;
use script_bindings::cell::DomRefCell;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGNumberListBinding::SVGNumberListMethods;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::element::Element;
use crate::dom::node::NodeTraits;
use crate::dom::svg::svganimationelement::trim_one_trailing_semicolon;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgnumber::SVGNumber;
use crate::dom::svg::svgtypes::{number_to_string, parse_number, split_list};

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGNumberList>
///
/// The base or animated value of an `SVGAnimatedNumberList`, which reflects an attribute.
/// The list is synchronized with the attribute whenever it or one of its numbers is used
/// after the attribute changed.
#[dom_struct]
pub(crate) struct SVGNumberList {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    read_only: bool,
    items: DomRefCell<Vec<Dom<SVGNumber>>>,
    /// The value of the attribute that the items were last synchronized with.
    synchronized_value: DomRefCell<Option<Option<String>>>,
}

/// Parse a list of numbers. An invalid list is an empty list.
fn parse_number_list(value: Option<&str>) -> Vec<f32> {
    value
        .and_then(|value| split_list(value)?.into_iter().map(parse_number).collect())
        .unwrap_or_default()
}

fn parse_animation_list(value: &str) -> Option<Vec<f32>> {
    split_list(value)?
        .into_iter()
        .map(parse_number)
        .collect::<Option<Vec<_>>>()
        .filter(|values| !values.is_empty())
}

fn interpolate_lists(from: &[f32], to: &[f32], fraction: f32) -> Vec<f32> {
    if from.len() != to.len() {
        return if fraction < 0.5 { from } else { to }.to_vec();
    }
    from.iter()
        .zip(to)
        .map(|(from, to)| from + (to - from) * fraction)
        .collect()
}

fn add_lists(underlying: &[f32], effect: &[f32]) -> Option<Vec<f32>> {
    if underlying.len() != effect.len() {
        return None;
    }
    Some(underlying.iter().zip(effect).map(|(a, b)| a + b).collect())
}

impl SVGNumberList {
    fn animated_values(&self, cx: &mut JSContext, mut result: Vec<f32>) -> Vec<f32> {
        for animation in self
            .element
            .owner_document()
            .smil_animations_targeting(cx, &self.element)
        {
            if animation.upcast::<Element>().local_name() != &local_name!("animate") {
                continue;
            }
            let element = animation.upcast::<SVGElement>();
            if element
                .svg_attribute(&local_name!("attributeName"))
                .as_deref() !=
                Some(self.attribute.as_ref())
            {
                continue;
            }
            let Some((progress, iteration)) = animation.sample_iteration() else {
                continue;
            };
            let additive =
                element.svg_attribute(&local_name!("additive")).as_deref() == Some("sum");
            let accumulate =
                element.svg_attribute(&local_name!("accumulate")).as_deref() == Some("sum");
            let parsed_values = element.svg_attribute(&local_name!("values")).map(|values| {
                trim_one_trailing_semicolon(&values)
                    .split(';')
                    .map(parse_animation_list)
                    .collect::<Option<Vec<_>>>()
            });
            let (from, to, by) = if parsed_values.is_some() {
                (None, None, None)
            } else {
                let parse = |name| {
                    element
                        .svg_attribute(&name)
                        .map(|value| parse_animation_list(&value))
                };
                (
                    parse(local_name!("from")),
                    parse(local_name!("to")),
                    parse(local_name!("by")),
                )
            };
            if parsed_values.as_ref().is_some_and(Option::is_none) ||
                from.as_ref().is_some_and(Option::is_none) ||
                to.as_ref().is_some_and(Option::is_none) ||
                by.as_ref().is_some_and(Option::is_none)
            {
                continue;
            }
            let (mut effect, additive, final_value) = if let Some(Some(values)) = parsed_values {
                let effect = if values.len() == 1 {
                    values[0].clone()
                } else {
                    let Some((index, fraction)) =
                        animation.sample_value_interval(values.len(), progress)
                    else {
                        continue;
                    };
                    interpolate_lists(&values[index], &values[index + 1], fraction)
                };
                (effect, additive, Some(values[values.len() - 1].clone()))
            } else {
                let Some((_, fraction)) = animation.sample_value_interval(2, progress) else {
                    continue;
                };
                match (from.flatten(), to.flatten(), by.flatten()) {
                    (Some(from), Some(to), _) => {
                        (interpolate_lists(&from, &to, fraction), additive, Some(to))
                    },
                    (Some(from), None, Some(by)) => {
                        let Some(to) = add_lists(&from, &by) else {
                            continue;
                        };
                        (interpolate_lists(&from, &to, fraction), additive, Some(to))
                    },
                    (None, None, Some(by)) => (
                        by.iter().map(|value| value * fraction).collect(),
                        true,
                        Some(by),
                    ),
                    (None, Some(to), _) => (interpolate_lists(&result, &to, fraction), false, None),
                    _ => continue,
                }
            };
            if accumulate && iteration > 0 {
                if let Some(final_value) = final_value {
                    let scaled = final_value
                        .iter()
                        .map(|value| value * iteration as f32)
                        .collect::<Vec<_>>();
                    let Some(sum) = add_lists(&effect, &scaled) else {
                        continue;
                    };
                    effect = sum;
                }
            }
            if additive {
                let Some(sum) = add_lists(&result, &effect) else {
                    continue;
                };
                result = sum;
            } else {
                result = effect;
            }
        }
        result
    }

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
        if !self.read_only && self.synchronized_value.borrow().as_ref() == Some(&value) {
            return;
        }
        let mut values = parse_number_list(value.as_deref());
        if self.read_only {
            values = self.animated_values(cx, values);
        }
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
            let item = SVGNumber::new(cx, &self.global(), value, Some(self), self.read_only);
            self.items.borrow_mut().push(Dom::from_ref(&*item));
        }
    }

    /// Serialize the items into the attribute.
    pub(crate) fn commit(&self, cx: &mut JSContext) {
        let value = self
            .items
            .borrow()
            .iter()
            .map(|item| number_to_string(item.raw_value()))
            .collect::<Vec<_>>()
            .join(" ");
        *self.synchronized_value.borrow_mut() = Some(Some(value.clone()));
        self.element.set_svg_attribute(cx, &self.attribute, value);
    }

    fn check_writable(&self) -> ErrorResult {
        if self.read_only {
            return Err(Error::NoModificationAllowed(Some(
                "This SVGNumberList is read-only".into(),
            )));
        }
        Ok(())
    }

    /// The item to insert for `new_item`: `new_item` itself, or a copy when it already
    /// belongs to a list.
    fn item_for_insertion(&self, cx: &mut JSContext, new_item: &SVGNumber) -> DomRoot<SVGNumber> {
        if new_item.is_attached() {
            return SVGNumber::new(
                cx,
                &self.global(),
                new_item.raw_value(),
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

impl SVGNumberListMethods<crate::DomTypeHolder> for SVGNumberList {
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
    fn Initialize(&self, cx: &mut JSContext, new_item: &SVGNumber) -> Fallible<DomRoot<SVGNumber>> {
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
    fn GetItem(&self, cx: &mut JSContext, index: u32) -> Fallible<DomRoot<SVGNumber>> {
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
    ) -> Fallible<Option<DomRoot<SVGNumber>>> {
        Ok(self.GetItem(cx, index).ok())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__insertItemBefore>
    fn InsertItemBefore(
        &self,
        cx: &mut JSContext,
        new_item: &SVGNumber,
        index: u32,
    ) -> Fallible<DomRoot<SVGNumber>> {
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
        new_item: &SVGNumber,
        index: u32,
    ) -> Fallible<DomRoot<SVGNumber>> {
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
    fn RemoveItem(&self, cx: &mut JSContext, index: u32) -> Fallible<DomRoot<SVGNumber>> {
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
    fn AppendItem(&self, cx: &mut JSContext, new_item: &SVGNumber) -> Fallible<DomRoot<SVGNumber>> {
        self.check_writable()?;
        self.synchronize(cx);
        let item = self.item_for_insertion(cx, new_item);
        self.items.borrow_mut().push(Dom::from_ref(&*item));
        self.commit(cx);
        Ok(item)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__setter>
    fn IndexedSetter(&self, cx: &mut JSContext, index: u32, new_item: &SVGNumber) -> ErrorResult {
        self.ReplaceItem(cx, new_item, index).map(|_| ())
    }
}
