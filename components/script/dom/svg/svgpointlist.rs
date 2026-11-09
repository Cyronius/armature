/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::local_name;
use js::context::JSContext;
use script_bindings::cell::DomRefCell;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGPointListBinding::SVGPointListMethods;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::dompoint::DOMPoint;
use crate::dom::element::Element;
use crate::dom::node::NodeTraits;
use crate::dom::svg::svganimationelement::trim_one_trailing_semicolon;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{is_svg_whitespace, number_to_string, parse_number_prefix};

/// <https://svgwg.org/svg2-draft/shapes.html#InterfaceSVGPointList>
///
/// The `points` or `animatedPoints` of a polyline or polygon, which reflect the `points`
/// attribute. Its elements are `DOMPoint`s, which the SVG DOM calls `SVGPoint`s. The list
/// is synchronized with the attribute whenever it or one of its points is used after the
/// attribute changed.
#[dom_struct]
pub(crate) struct SVGPointList {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    read_only: bool,
    items: DomRefCell<Vec<Dom<DOMPoint>>>,
    /// The value of the attribute that the items were last synchronized with.
    synchronized_value: DomRefCell<Option<Option<String>>>,
}

/// Parse a list of points: coordinates separated by whitespace and optionally a comma.
/// An invalid coordinate clears the list, while an odd last coordinate is ignored.
///
/// <https://svgwg.org/svg2-draft/shapes.html#DataTypePoints>
pub(crate) fn parse_point_list(value: Option<&str>) -> Vec<(f64, f64)> {
    let Some(value) = value else {
        return Vec::new();
    };
    let mut coordinates = Vec::new();
    let mut rest = value.trim_start_matches(is_svg_whitespace);
    while !rest.is_empty() {
        let Some((number, after)) = parse_number_prefix(rest) else {
            return Vec::new();
        };
        coordinates.push(number as f64);
        rest = after.trim_start_matches(is_svg_whitespace);
        if let Some(after_comma) = rest.strip_prefix(',') {
            rest = after_comma.trim_start_matches(is_svg_whitespace);
        }
    }
    coordinates
        .chunks_exact(2)
        .map(|pair| (pair[0], pair[1]))
        .collect()
}

/// Animation values must be valid in their entirety, unlike the base attribute's
/// error recovery, which retains points preceding a malformed coordinate.
fn parse_animation_point_list(value: &str) -> Option<Vec<(f64, f64)>> {
    let mut coordinates = Vec::new();
    let mut rest = value.trim_start_matches(is_svg_whitespace);
    while !rest.is_empty() {
        let (number, after) = parse_number_prefix(rest)?;
        if !number.is_finite() {
            return None;
        }
        coordinates.push(number as f64);
        rest = after.trim_start_matches(is_svg_whitespace);
        if let Some(after_comma) = rest.strip_prefix(',') {
            rest = after_comma.trim_start_matches(is_svg_whitespace);
            if rest.is_empty() {
                return None;
            }
        }
    }
    if coordinates.is_empty() || coordinates.len() % 2 != 0 {
        return None;
    }
    Some(
        coordinates
            .chunks_exact(2)
            .map(|pair| (pair[0], pair[1]))
            .collect(),
    )
}

fn interpolate_lists(from: &[(f64, f64)], to: &[(f64, f64)], fraction: f32) -> Vec<(f64, f64)> {
    if from.len() != to.len() {
        return if fraction < 0.5 { from } else { to }.to_vec();
    }
    from.iter()
        .zip(to)
        .map(|(&(from_x, from_y), &(to_x, to_y))| {
            (
                from_x + (to_x - from_x) * f64::from(fraction),
                from_y + (to_y - from_y) * f64::from(fraction),
            )
        })
        .collect()
}

fn add_lists(underlying: &[(f64, f64)], effect: &[(f64, f64)]) -> Option<Vec<(f64, f64)>> {
    if underlying.len() != effect.len() {
        return None;
    }
    Some(
        underlying
            .iter()
            .zip(effect)
            .map(|(&(x, y), &(dx, dy))| (x + dx, y + dy))
            .collect(),
    )
}

impl SVGPointList {
    fn animated_values(&self, cx: &mut JSContext, mut result: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
        for animation in self
            .element
            .owner_document()
            .smil_animations_targeting(cx, &self.element)
        {
            let child = animation.upcast::<Element>();
            if child.local_name() != &local_name!("animate") &&
                child.local_name() != &local_name!("set")
            {
                continue;
            }
            let element = animation.upcast::<SVGElement>();
            if element
                .svg_attribute(&local_name!("attributeName"))
                .as_deref() !=
                Some("points")
            {
                continue;
            }
            // `points` is an XML attribute, not a CSS property.
            if element
                .svg_attribute(&local_name!("attributeType"))
                .as_deref() ==
                Some("CSS")
            {
                continue;
            }
            if child.local_name() == &local_name!("set") {
                if animation.set_is_active() &&
                    let Some(to) = element
                        .svg_attribute(&local_name!("to"))
                        .as_deref()
                        .and_then(parse_animation_point_list)
                {
                    result = to;
                }
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
                    .map(parse_animation_point_list)
                    .collect::<Option<Vec<_>>>()
            });
            let (from, to, by) = if parsed_values.is_some() {
                (None, None, None)
            } else {
                let parse = |name| {
                    element
                        .svg_attribute(&name)
                        .map(|value| parse_animation_point_list(&value))
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
                        by.iter()
                            .map(|&(x, y)| (x * f64::from(fraction), y * f64::from(fraction)))
                            .collect(),
                        true,
                        Some(by),
                    ),
                    (None, Some(to), _) => (interpolate_lists(&result, &to, fraction), false, None),
                    _ => continue,
                }
            };
            if accumulate &&
                iteration > 0 &&
                let Some(final_value) = final_value
            {
                let scaled = final_value
                    .iter()
                    .map(|&(x, y)| (x * f64::from(iteration), y * f64::from(iteration)))
                    .collect::<Vec<_>>();
                let Some(sum) = add_lists(&effect, &scaled) else {
                    continue;
                };
                effect = sum;
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

    pub(crate) fn new(cx: &mut JSContext, element: &SVGElement, read_only: bool) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                element: Dom::from_ref(element),
                read_only,
                items: Default::default(),
                synchronized_value: Default::default(),
            }),
            &*element.owner_window(),
        )
    }

    /// <https://svgwg.org/svg2-draft/types.html#TermSynchronizeList>
    pub(crate) fn synchronize(&self, cx: &mut JSContext) {
        let value = self.element.svg_attribute(&local_name!("points"));
        if !self.read_only && self.synchronized_value.borrow().as_ref() == Some(&value) {
            return;
        }
        let mut values = parse_point_list(value.as_deref());
        if self.read_only {
            values = self.animated_values(cx, values);
        }
        *self.synchronized_value.borrow_mut() = Some(value);

        // > If the new value has fewer items than the list, detach the items beyond its
        // > length and remove them. Set the value of each remaining item.
        let existing = {
            let mut items = self.items.borrow_mut();
            if values.len() < items.len() {
                let removed: Vec<_> = items
                    .drain(values.len()..)
                    .map(|item| item.as_rooted())
                    .collect();
                for item in removed {
                    item.detach_from_svg_list();
                }
            }
            for (item, (x, y)) in items.iter().zip(values.iter()) {
                item.set_raw_coordinates(*x, *y);
            }
            items.len()
        };
        // > Append new items for the rest of the values.
        for (x, y) in values.into_iter().skip(existing) {
            let item = DOMPoint::new_svg_list_item(cx, &self.global(), x, y, self, self.read_only);
            self.items.borrow_mut().push(Dom::from_ref(&*item));
        }
    }

    /// Serialize the items into the attribute.
    pub(crate) fn commit(&self, cx: &mut JSContext) {
        let value = self
            .items
            .borrow()
            .iter()
            .map(|item| {
                format!(
                    "{},{}",
                    number_to_string(item.x() as f32),
                    number_to_string(item.y() as f32)
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        *self.synchronized_value.borrow_mut() = Some(Some(value.clone()));
        self.element
            .set_svg_attribute(cx, &local_name!("points"), value);
    }

    fn check_writable(&self) -> ErrorResult {
        if self.read_only {
            return Err(Error::NoModificationAllowed(Some(
                "This SVGPointList is read-only".into(),
            )));
        }
        Ok(())
    }

    /// The item to insert for `new_item`: `new_item` itself, or a copy when it already
    /// belongs to a list.
    fn item_for_insertion(&self, cx: &mut JSContext, new_item: &DOMPoint) -> DomRoot<DOMPoint> {
        if new_item.is_attached_to_svg_list() {
            return DOMPoint::new_svg_list_item(
                cx,
                &self.global(),
                new_item.x(),
                new_item.y(),
                self,
                self.read_only,
            );
        }
        new_item.attach_to_svg_list(self, self.read_only);
        DomRoot::from_ref(new_item)
    }

    fn index_error() -> Error {
        Error::IndexSize(Some("The index is out of range".into()))
    }

    fn detach_all(&self) {
        let removed: Vec<_> = self
            .items
            .borrow_mut()
            .drain(..)
            .map(|item| item.as_rooted())
            .collect();
        for item in removed {
            item.detach_from_svg_list();
        }
    }
}

impl SVGPointListMethods<crate::DomTypeHolder> for SVGPointList {
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
        self.detach_all();
        self.commit(cx);
        Ok(())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__initialize>
    fn Initialize(&self, cx: &mut JSContext, new_item: &DOMPoint) -> Fallible<DomRoot<DOMPoint>> {
        self.check_writable()?;
        self.synchronize(cx);
        self.detach_all();
        let item = self.item_for_insertion(cx, new_item);
        self.items.borrow_mut().push(Dom::from_ref(&*item));
        self.commit(cx);
        Ok(item)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__getItem>
    fn GetItem(&self, cx: &mut JSContext, index: u32) -> Fallible<DomRoot<DOMPoint>> {
        self.synchronize(cx);
        self.items
            .borrow()
            .get(index as usize)
            .map(|item| item.as_rooted())
            .ok_or_else(Self::index_error)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__getItem>
    fn IndexedGetter(&self, cx: &mut JSContext, index: u32) -> Fallible<Option<DomRoot<DOMPoint>>> {
        Ok(self.GetItem(cx, index).ok())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__insertItemBefore>
    fn InsertItemBefore(
        &self,
        cx: &mut JSContext,
        new_item: &DOMPoint,
        index: u32,
    ) -> Fallible<DomRoot<DOMPoint>> {
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
        new_item: &DOMPoint,
        index: u32,
    ) -> Fallible<DomRoot<DOMPoint>> {
        self.check_writable()?;
        self.synchronize(cx);
        if index as usize >= self.items.borrow().len() {
            return Err(Self::index_error());
        }
        let item = self.item_for_insertion(cx, new_item);
        let old = self.items.borrow()[index as usize].as_rooted();
        self.items.borrow_mut()[index as usize] = Dom::from_ref(&*item);
        if *old != *item {
            old.detach_from_svg_list();
        }
        self.commit(cx);
        Ok(item)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__removeItem>
    fn RemoveItem(&self, cx: &mut JSContext, index: u32) -> Fallible<DomRoot<DOMPoint>> {
        self.check_writable()?;
        self.synchronize(cx);
        if index as usize >= self.items.borrow().len() {
            return Err(Self::index_error());
        }
        let item = self.items.borrow_mut().remove(index as usize).as_rooted();
        item.detach_from_svg_list();
        self.commit(cx);
        Ok(item)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__appendItem>
    fn AppendItem(&self, cx: &mut JSContext, new_item: &DOMPoint) -> Fallible<DomRoot<DOMPoint>> {
        self.check_writable()?;
        self.synchronize(cx);
        let item = self.item_for_insertion(cx, new_item);
        self.items.borrow_mut().push(Dom::from_ref(&*item));
        self.commit(cx);
        Ok(item)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNameList__setter>
    fn IndexedSetter(&self, cx: &mut JSContext, index: u32, new_item: &DOMPoint) -> ErrorResult {
        self.ReplaceItem(cx, new_item, index).map(|_| ())
    }
}
