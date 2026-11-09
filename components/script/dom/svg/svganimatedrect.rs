/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::LocalName;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedRectBinding::SVGAnimatedRectMethods;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::domrect::DOMRect;
use crate::dom::node::NodeTraits;
use crate::dom::svg::svganimationelement::{
    sample_numeric_animation_with_by, sample_targeted_numeric_animation_with_by,
};
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{number_to_string, parse_number, split_list};

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedRect>
///
/// Its base and animated values are `DOMRect`s (which the SVG DOM calls `SVGRect`) whose
/// coordinates reflect the attribute.
#[dom_struct]
pub(crate) struct SVGAnimatedRect {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    base_val: MutNullableDom<DOMRect>,
    anim_val: MutNullableDom<DOMRect>,
}

impl SVGAnimatedRect {
    pub(crate) fn new(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: LocalName,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                element: Dom::from_ref(element),
                attribute,
                base_val: Default::default(),
                anim_val: Default::default(),
            }),
            &*element.owner_window(),
        )
    }

    /// The rectangle, parsed from the attribute: four numbers, of which the width and height
    /// must not be negative. A missing or invalid attribute gives an empty rectangle.
    fn parse_rect(value: &str) -> Option<[f32; 4]> {
        Self::parse_rect_delta(value).filter(|rect| rect[2] >= 0.0 && rect[3] >= 0.0)
    }

    /// A `by` value is a signed change to each coordinate, including width and height.
    fn parse_rect_delta(value: &str) -> Option<[f32; 4]> {
        let numbers = split_list(value)?
            .into_iter()
            .map(parse_number)
            .collect::<Option<Vec<f32>>>()?;
        match numbers.as_slice() {
            [x, y, width, height] => Some([*x, *y, *width, *height]),
            _ => None,
        }
    }

    pub(crate) fn rect(&self) -> [f64; 4] {
        self.element
            .svg_attribute(&self.attribute)
            .as_deref()
            .and_then(Self::parse_rect)
            .unwrap_or_default()
            .map(f64::from)
    }

    pub(crate) fn animated_rect(&self, cx: &mut JSContext) -> [f64; 4] {
        let base = self.rect();
        std::array::from_fn(|index| {
            f64::from(sample_targeted_numeric_animation_with_by(
                cx,
                &self.element,
                &self.attribute,
                base[index] as f32,
                |value| Self::parse_rect(value).map(|rect| rect[index]),
                |value| Self::parse_rect_delta(value).map(|rect| rect[index]),
            ))
        })
    }

    /// The child-animation value for contexts that cannot resolve external targets.
    pub(crate) fn animated_rect_without_context(&self) -> [f64; 4] {
        let base = self.rect();
        std::array::from_fn(|index| {
            f64::from(sample_numeric_animation_with_by(
                &self.element,
                &self.attribute,
                base[index] as f32,
                |value| Self::parse_rect(value).map(|rect| rect[index]),
                |value| Self::parse_rect_delta(value).map(|rect| rect[index]),
            ))
        })
    }

    /// Set the coordinate with index `index` (x, y, width, height) of the rectangle.
    pub(crate) fn set_coordinate(&self, cx: &mut JSContext, index: usize, value: f64) {
        let mut rect = self.rect();
        rect[index] = value;
        let value = rect
            .iter()
            .map(|number| number_to_string(*number as f32))
            .collect::<Vec<_>>()
            .join(" ");
        self.element.set_svg_attribute(cx, &self.attribute, value);
    }
}

impl SVGAnimatedRectMethods<crate::DomTypeHolder> for SVGAnimatedRect {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedRect__baseVal>
    fn BaseVal(&self, cx: &mut JSContext) -> DomRoot<DOMRect> {
        if let Some(rect) = self.base_val.get() {
            return rect;
        }
        let rect = DOMRect::new_reflecting_svg_rect(cx, self, false);
        self.base_val.set(Some(&rect));
        rect
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedRect__animVal>
    fn AnimVal(&self, cx: &mut JSContext) -> DomRoot<DOMRect> {
        if let Some(rect) = self.anim_val.get() {
            return rect;
        }
        let rect = DOMRect::new_reflecting_svg_rect(cx, self, true);
        self.anim_val.set(Some(&rect));
        rect
    }
}
