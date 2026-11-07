/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::LocalName;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedIntegerBinding::SVGAnimatedIntegerMethods;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::node::NodeTraits;
use crate::dom::svg::svganimatednumber::SVGNumberPart;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{parse_integer, parse_integer_optional_integer};

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedInteger>
#[dom_struct]
pub(crate) struct SVGAnimatedInteger {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    part: SVGNumberPart,
    /// The value when the attribute is missing or invalid.
    initial: i32,
}

impl SVGAnimatedInteger {
    pub(crate) fn new(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: LocalName,
        part: SVGNumberPart,
        initial: i32,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                element: Dom::from_ref(element),
                attribute,
                part,
                initial,
            }),
            &*element.owner_window(),
        )
    }

    fn pair(&self) -> Option<(i32, i32)> {
        parse_integer_optional_integer(&self.element.svg_attribute(&self.attribute)?)
    }

    fn value(&self) -> i32 {
        let value = match self.part {
            SVGNumberPart::Whole | SVGNumberPart::WholeOrPercentage => self
                .element
                .svg_attribute(&self.attribute)
                .and_then(|value| parse_integer(&value)),
            SVGNumberPart::First => self.pair().map(|(first, _)| first),
            SVGNumberPart::Second => self.pair().map(|(_, second)| second),
        };
        value.unwrap_or(self.initial)
    }
}

impl SVGAnimatedIntegerMethods<crate::DomTypeHolder> for SVGAnimatedInteger {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedInteger__baseVal>
    fn BaseVal(&self) -> i32 {
        self.value()
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedInteger__baseVal>
    fn SetBaseVal(&self, cx: &mut JSContext, value: i32) {
        let string = match self.part {
            SVGNumberPart::Whole | SVGNumberPart::WholeOrPercentage => value.to_string(),
            SVGNumberPart::First | SVGNumberPart::Second => {
                let (first, second) = self.pair().unwrap_or((self.initial, self.initial));
                if self.part == SVGNumberPart::First {
                    format!("{value} {second}")
                } else {
                    format!("{first} {value}")
                }
            },
        };
        self.element.set_svg_attribute(cx, &self.attribute, string);
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedInteger__animVal>
    fn AnimVal(&self) -> i32 {
        self.value()
    }
}
