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
    pub(crate) fn rect(&self) -> [f64; 4] {
        self.element
            .svg_attribute(&self.attribute)
            .and_then(|value| {
                let numbers = split_list(&value)?
                    .into_iter()
                    .map(parse_number)
                    .collect::<Option<Vec<f32>>>()?;
                match numbers.as_slice() {
                    [x, y, width, height] if *width >= 0.0 && *height >= 0.0 => {
                        Some([*x as f64, *y as f64, *width as f64, *height as f64])
                    },
                    _ => None,
                }
            })
            .unwrap_or_default()
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
