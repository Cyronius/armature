/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, local_name};
use js::context::JSContext;
use script_bindings::cell::DomRefCell;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGLengthListBinding::SVGLengthListMethods;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::element::Element;
use crate::dom::node::NodeTraits;
use crate::dom::svg::svganimationelement::trim_one_trailing_semicolon;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svglength::{ElementLengthContext, SVGLength};
use crate::dom::svg::svgtypes::{SVGLengthContext, SVGLengthDirection, SVGLengthValue, split_list};

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGLengthList>
///
/// The base or animated value of an `SVGAnimatedLengthList`, which reflects an attribute.
/// The list is synchronized with the attribute whenever it or one of its numbers is used
/// after the attribute changed.
#[dom_struct]
pub(crate) struct SVGLengthList {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    direction: SVGLengthDirection,
    read_only: bool,
    items: DomRefCell<Vec<Dom<SVGLength>>>,
    /// The value of the attribute that the items were last synchronized with.
    synchronized_value: DomRefCell<Option<Option<String>>>,
}

/// Parse a list of lengths. An invalid list is an empty list.
fn parse_length_list(value: Option<&str>) -> Vec<SVGLengthValue> {
    value
        .and_then(|value| {
            split_list(value)?
                .into_iter()
                .map(SVGLengthValue::parse)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_animation_list(value: &str) -> Option<Vec<SVGLengthValue>> {
    split_list(value)?
        .into_iter()
        .map(SVGLengthValue::parse)
        .collect::<Option<Vec<_>>>()
        .filter(|values| !values.is_empty())
}

fn interpolate_lists(
    cx: &mut JSContext,
    context: &dyn SVGLengthContext,
    from: &[SVGLengthValue],
    to: &[SVGLengthValue],
    fraction: f32,
) -> Option<Vec<SVGLengthValue>> {
    if from.len() != to.len() {
        return Some(if fraction < 0.5 { from } else { to }.to_vec());
    }
    from.iter()
        .zip(to)
        .map(|(from, to)| {
            if from.unit_type == to.unit_type {
                return Some(SVGLengthValue::new(
                    from.value + (to.value - from.value) * fraction,
                    from.unit_type,
                ));
            }
            let from_units = from.to_user_units(cx, context)?;
            let to_units = to.to_user_units(cx, context)?;
            let unit_type = if fraction <= 0.5 {
                from.unit_type
            } else {
                to.unit_type
            };
            SVGLengthValue::from_user_units(
                cx,
                from_units + (to_units - from_units) * fraction,
                unit_type,
                context,
            )
        })
        .collect()
}

fn add_lists(
    cx: &mut JSContext,
    context: &dyn SVGLengthContext,
    underlying: &[SVGLengthValue],
    effect: &[SVGLengthValue],
) -> Option<Vec<SVGLengthValue>> {
    if underlying.len() != effect.len() {
        return None;
    }
    underlying
        .iter()
        .zip(effect)
        .map(|(underlying, effect)| {
            let sum = underlying.to_user_units(cx, context)? + effect.to_user_units(cx, context)?;
            SVGLengthValue::from_user_units(cx, sum, underlying.unit_type, context)
        })
        .collect()
}

impl SVGLengthList {
    pub(crate) fn new(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: LocalName,
        direction: SVGLengthDirection,
        read_only: bool,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                element: Dom::from_ref(element),
                attribute,
                direction,
                read_only,
                items: Default::default(),
                synchronized_value: Default::default(),
            }),
            &*element.owner_window(),
        )
    }

    pub(crate) fn element(&self) -> &SVGElement {
        &self.element
    }

    pub(crate) fn direction(&self) -> SVGLengthDirection {
        self.direction
    }

    fn animated_values(
        &self,
        cx: &mut JSContext,
        mut result: Vec<SVGLengthValue>,
    ) -> Vec<SVGLengthValue> {
        let context = ElementLengthContext {
            element: &self.element,
            direction: self.direction,
        };
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
                    let Some(effect) = interpolate_lists(
                        cx,
                        &context,
                        &values[index],
                        &values[index + 1],
                        fraction,
                    ) else {
                        continue;
                    };
                    effect
                };
                (effect, additive, Some(values[values.len() - 1].clone()))
            } else {
                let (Some((_, fraction)), from, to, by) = (
                    animation.sample_value_interval(2, progress),
                    from.flatten(),
                    to.flatten(),
                    by.flatten(),
                ) else {
                    continue;
                };
                match (from, to, by) {
                    (Some(from), Some(to), _) => {
                        let Some(effect) = interpolate_lists(cx, &context, &from, &to, fraction)
                        else {
                            continue;
                        };
                        (effect, additive, Some(to))
                    },
                    (Some(from), None, Some(by)) => {
                        let Some(to) = add_lists(cx, &context, &from, &by) else {
                            continue;
                        };
                        let Some(effect) = interpolate_lists(cx, &context, &from, &to, fraction)
                        else {
                            continue;
                        };
                        (effect, additive, Some(to))
                    },
                    (None, None, Some(by)) => {
                        let effect = by
                            .iter()
                            .map(|value| {
                                SVGLengthValue::new(value.value * fraction, value.unit_type)
                            })
                            .collect();
                        (effect, true, Some(by))
                    },
                    (None, Some(to), _) => {
                        let Some(effect) = interpolate_lists(cx, &context, &result, &to, fraction)
                        else {
                            continue;
                        };
                        (effect, false, None)
                    },
                    _ => continue,
                }
            };
            if accumulate && iteration > 0 {
                if let Some(final_value) = final_value {
                    let scaled = final_value
                        .iter()
                        .map(|value| {
                            SVGLengthValue::new(value.value * iteration as f32, value.unit_type)
                        })
                        .collect::<Vec<_>>();
                    let Some(sum) = add_lists(cx, &context, &effect, &scaled) else {
                        continue;
                    };
                    effect = sum;
                }
            }
            if additive {
                let Some(sum) = add_lists(cx, &context, &result, &effect) else {
                    continue;
                };
                result = sum;
            } else {
                result = effect;
            }
        }
        result
    }

    /// <https://svgwg.org/svg2-draft/types.html#TermSynchronizeList>
    pub(crate) fn synchronize(&self, cx: &mut JSContext) {
        let value = self.element.svg_attribute(&self.attribute);
        if !self.read_only && self.synchronized_value.borrow().as_ref() == Some(&value) {
            return;
        }
        let mut values = parse_length_list(value.as_deref());
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
                item.set_raw_length(*value);
            }
            items.len()
        };
        // > Append new items for the rest of the values.
        for value in values.into_iter().skip(existing) {
            let item = SVGLength::new_own(cx, &self.global(), value, Some(self), self.read_only);
            self.items.borrow_mut().push(Dom::from_ref(&*item));
        }
    }

    /// Serialize the items into the attribute.
    pub(crate) fn commit(&self, cx: &mut JSContext) {
        let value = self
            .items
            .borrow()
            .iter()
            .map(|item| item.length(cx).to_string())
            .collect::<Vec<_>>()
            .join(" ");
        *self.synchronized_value.borrow_mut() = Some(Some(value.clone()));
        self.element.set_svg_attribute(cx, &self.attribute, value);
    }

    fn check_writable(&self) -> ErrorResult {
        if self.read_only {
            return Err(Error::NoModificationAllowed(Some(
                "This SVGLengthList is read-only".into(),
            )));
        }
        Ok(())
    }

    /// The item to insert for `new_item`: `new_item` itself, or a copy when it already
    /// belongs to a list.
    fn item_for_insertion(&self, cx: &mut JSContext, new_item: &SVGLength) -> DomRoot<SVGLength> {
        if new_item.is_attached() {
            let value = new_item.length(cx);
            return SVGLength::new_own(cx, &self.global(), value, Some(self), self.read_only);
        }
        new_item.attach(self, self.read_only);
        DomRoot::from_ref(new_item)
    }

    fn index_error() -> Error {
        Error::IndexSize(Some("The index is out of range".into()))
    }
}

impl SVGLengthListMethods<crate::DomTypeHolder> for SVGLengthList {
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
    fn Initialize(&self, cx: &mut JSContext, new_item: &SVGLength) -> Fallible<DomRoot<SVGLength>> {
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
    fn GetItem(&self, cx: &mut JSContext, index: u32) -> Fallible<DomRoot<SVGLength>> {
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
    ) -> Fallible<Option<DomRoot<SVGLength>>> {
        Ok(self.GetItem(cx, index).ok())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__insertItemBefore>
    fn InsertItemBefore(
        &self,
        cx: &mut JSContext,
        new_item: &SVGLength,
        index: u32,
    ) -> Fallible<DomRoot<SVGLength>> {
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
        new_item: &SVGLength,
        index: u32,
    ) -> Fallible<DomRoot<SVGLength>> {
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
    fn RemoveItem(&self, cx: &mut JSContext, index: u32) -> Fallible<DomRoot<SVGLength>> {
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
    fn AppendItem(&self, cx: &mut JSContext, new_item: &SVGLength) -> Fallible<DomRoot<SVGLength>> {
        self.check_writable()?;
        self.synchronize(cx);
        let item = self.item_for_insertion(cx, new_item);
        self.items.borrow_mut().push(Dom::from_ref(&*item));
        self.commit(cx);
        Ok(item)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__setter>
    fn IndexedSetter(&self, cx: &mut JSContext, index: u32, new_item: &SVGLength) -> ErrorResult {
        self.ReplaceItem(cx, new_item, index).map(|_| ())
    }
}
