/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::LocalName;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedLengthBinding::SVGAnimatedLengthMethods;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::node::NodeTraits;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svglength::SVGLength;
use crate::dom::svg::svgtypes::{SVGLengthDirection, SVGLengthValue};

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedLength>
///
/// The base value is not stored: it is parsed from the content attribute when it is read,
/// and serialized into the content attribute when it is changed, so that the two always
/// agree. Servo does not run SVG animations, so the animated value is the base value.
#[dom_struct]
pub(crate) struct SVGAnimatedLength {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    direction: SVGLengthDirection,
    /// The value when the attribute is missing or invalid.
    initial: SVGLengthValue,
    base_val: MutNullableDom<SVGLength>,
    anim_val: MutNullableDom<SVGLength>,
}

impl SVGAnimatedLength {
    fn new_inherited(
        element: &SVGElement,
        attribute: LocalName,
        direction: SVGLengthDirection,
        initial: SVGLengthValue,
    ) -> Self {
        Self {
            reflector_: Reflector::new(),
            element: Dom::from_ref(element),
            attribute,
            direction,
            initial,
            base_val: Default::default(),
            anim_val: Default::default(),
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: LocalName,
        direction: SVGLengthDirection,
        initial: SVGLengthValue,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self::new_inherited(element, attribute, direction, initial)),
            &*element.owner_window(),
        )
    }

    pub(crate) fn element(&self) -> &SVGElement {
        &self.element
    }

    pub(crate) fn direction(&self) -> SVGLengthDirection {
        self.direction
    }

    /// The base value, parsed from the content attribute.
    pub(crate) fn base_value(&self) -> SVGLengthValue {
        // Negative values of the lengths that cannot be negative are invalid.
        let non_negative = matches!(
            &*self.attribute,
            "width" | "height" | "r" | "rx" | "ry" | "fr" | "markerWidth" | "markerHeight"
        );
        self.element
            .svg_attribute(&self.attribute)
            .and_then(|value| SVGLengthValue::parse(&value))
            .filter(|value| !non_negative || value.value >= 0.0)
            .unwrap_or(self.initial)
    }

    /// Set the base value, by setting the content attribute.
    pub(crate) fn set_base_value(&self, cx: &mut JSContext, value: SVGLengthValue) {
        self.element
            .set_svg_attribute(cx, &self.attribute, value.to_string());
    }
}

impl SVGAnimatedLengthMethods<crate::DomTypeHolder> for SVGAnimatedLength {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedLength__baseVal>
    fn BaseVal(&self, cx: &mut JSContext) -> DomRoot<SVGLength> {
        if let Some(length) = self.base_val.get() {
            return length;
        }
        let length = SVGLength::new_reflected(cx, self, false);
        self.base_val.set(Some(&length));
        length
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedLength__animVal>
    fn AnimVal(&self, cx: &mut JSContext) -> DomRoot<SVGLength> {
        if let Some(length) = self.anim_val.get() {
            return length;
        }
        let length = SVGLength::new_reflected(cx, self, true);
        self.anim_val.set(Some(&length));
        length
    }
}
