/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::local_name;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedPreserveAspectRatioBinding::SVGAnimatedPreserveAspectRatioMethods;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::node::NodeTraits;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgpreserveaspectratio::SVGPreserveAspectRatio;
use crate::dom::svg::svgtypes::is_svg_whitespace;

/// The keywords of the `<align>` values, indexed by alignment type.
const ALIGN_KEYWORDS: [&str; 11] = [
    "", "none", "xMinYMin", "xMidYMin", "xMaxYMin", "xMinYMid", "xMidYMid", "xMaxYMid", "xMinYMax",
    "xMidYMax", "xMaxYMax",
];
pub(crate) const SVG_PRESERVEASPECTRATIO_XMIDYMID: u16 = 6;
pub(crate) const SVG_PRESERVEASPECTRATIO_XMAXYMAX: u16 = 10;
pub(crate) const SVG_MEETORSLICE_MEET: u16 = 1;
pub(crate) const SVG_MEETORSLICE_SLICE: u16 = 2;

/// <https://svgwg.org/svg2-draft/coords.html#InterfaceSVGAnimatedPreserveAspectRatio>
#[dom_struct]
pub(crate) struct SVGAnimatedPreserveAspectRatio {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    base_val: MutNullableDom<SVGPreserveAspectRatio>,
    anim_val: MutNullableDom<SVGPreserveAspectRatio>,
}

impl SVGAnimatedPreserveAspectRatio {
    pub(crate) fn new(cx: &mut JSContext, element: &SVGElement) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                element: Dom::from_ref(element),
                base_val: Default::default(),
                anim_val: Default::default(),
            }),
            &*element.owner_window(),
        )
    }

    /// The alignment and the meet-or-slice type, parsed from the `preserveAspectRatio`
    /// attribute.
    ///
    /// <https://svgwg.org/svg2-draft/coords.html#PreserveAspectRatioAttribute>
    pub(crate) fn value(&self) -> (u16, u16) {
        self.element
            .svg_attribute(&local_name!("preserveAspectRatio"))
            .and_then(|value| {
                let mut tokens = value
                    .split(is_svg_whitespace)
                    .filter(|token| !token.is_empty());
                let first = tokens.next()?;
                let align = ALIGN_KEYWORDS
                    .iter()
                    .skip(1)
                    .position(|keyword| *keyword == first)? as u16 +
                    1;
                let meet_or_slice = match tokens.next() {
                    None | Some("meet") => SVG_MEETORSLICE_MEET,
                    Some("slice") => SVG_MEETORSLICE_SLICE,
                    Some(_) => return None,
                };
                if tokens.next().is_some() {
                    return None;
                }
                Some((align, meet_or_slice))
            })
            .unwrap_or((SVG_PRESERVEASPECTRATIO_XMIDYMID, SVG_MEETORSLICE_MEET))
    }

    /// Set the `preserveAspectRatio` attribute.
    pub(crate) fn set_value(&self, cx: &mut JSContext, align: u16, meet_or_slice: u16) {
        let mut value = ALIGN_KEYWORDS[align as usize].to_owned();
        if meet_or_slice == SVG_MEETORSLICE_SLICE {
            value.push_str(" slice");
        }
        self.element
            .set_svg_attribute(cx, &local_name!("preserveAspectRatio"), value);
    }
}

impl SVGAnimatedPreserveAspectRatioMethods<crate::DomTypeHolder>
    for SVGAnimatedPreserveAspectRatio
{
    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGAnimatedPreserveAspectRatio__baseVal>
    fn BaseVal(&self, cx: &mut JSContext) -> DomRoot<SVGPreserveAspectRatio> {
        if let Some(value) = self.base_val.get() {
            return value;
        }
        let value = SVGPreserveAspectRatio::new(cx, self, false);
        self.base_val.set(Some(&value));
        value
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGAnimatedPreserveAspectRatio__animVal>
    fn AnimVal(&self, cx: &mut JSContext) -> DomRoot<SVGPreserveAspectRatio> {
        if let Some(value) = self.anim_val.get() {
            return value;
        }
        let value = SVGPreserveAspectRatio::new(cx, self, true);
        self.anim_val.set(Some(&value));
        value
    }
}
