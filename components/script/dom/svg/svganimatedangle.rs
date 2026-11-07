/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::LocalName;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedAngleBinding::SVGAnimatedAngleMethods;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::node::NodeTraits;
use crate::dom::svg::svgangle::SVGAngle;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{SVG_ANGLETYPE_UNSPECIFIED, SVGAngleValue};

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedAngle>
///
/// Reflects an attribute whose value may be an angle, like the `orient` attribute of a
/// marker, which may also be `auto`. When the attribute is not an angle, the angle is zero.
#[dom_struct]
pub(crate) struct SVGAnimatedAngle {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    base_val: MutNullableDom<SVGAngle>,
    anim_val: MutNullableDom<SVGAngle>,
}

impl SVGAnimatedAngle {
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

    pub(crate) fn element(&self) -> &SVGElement {
        &self.element
    }

    /// The base value, parsed from the attribute.
    pub(crate) fn base_value(&self) -> SVGAngleValue {
        self.element
            .svg_attribute(&self.attribute)
            .and_then(|value| SVGAngleValue::parse(&value))
            .unwrap_or(SVGAngleValue::new(0.0, SVG_ANGLETYPE_UNSPECIFIED))
    }

    /// Set the base value, by setting the attribute.
    pub(crate) fn set_base_value(&self, cx: &mut JSContext, value: SVGAngleValue) {
        self.element
            .set_svg_attribute(cx, &self.attribute, value.to_string());
    }
}

impl SVGAnimatedAngleMethods<crate::DomTypeHolder> for SVGAnimatedAngle {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedAngle__baseVal>
    fn BaseVal(&self, cx: &mut JSContext) -> DomRoot<SVGAngle> {
        if let Some(angle) = self.base_val.get() {
            return angle;
        }
        let angle = SVGAngle::new_reflected(cx, self, false);
        self.base_val.set(Some(&angle));
        angle
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedAngle__animVal>
    fn AnimVal(&self, cx: &mut JSContext) -> DomRoot<SVGAngle> {
        if let Some(angle) = self.anim_val.get() {
            return angle;
        }
        let angle = SVGAngle::new_reflected(cx, self, true);
        self.anim_val.set(Some(&angle));
        angle
    }
}
