/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::LocalName;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedBooleanBinding::SVGAnimatedBooleanMethods;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::node::NodeTraits;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::is_svg_whitespace;

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedBoolean>
#[dom_struct]
pub(crate) struct SVGAnimatedBoolean {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
}

impl SVGAnimatedBoolean {
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
            }),
            &*element.owner_window(),
        )
    }

    fn value(&self) -> bool {
        self.element
            .svg_attribute(&self.attribute)
            .is_some_and(|value| value.trim_matches(is_svg_whitespace) == "true")
    }
}

impl SVGAnimatedBooleanMethods<crate::DomTypeHolder> for SVGAnimatedBoolean {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedBoolean__baseVal>
    fn BaseVal(&self) -> bool {
        self.value()
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedBoolean__baseVal>
    fn SetBaseVal(&self, cx: &mut JSContext, value: bool) {
        self.element
            .set_svg_attribute(cx, &self.attribute, value.to_string());
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedBoolean__animVal>
    fn AnimVal(&self) -> bool {
        self.value()
    }
}
