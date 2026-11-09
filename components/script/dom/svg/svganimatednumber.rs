/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::LocalName;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedNumberBinding::SVGAnimatedNumberMethods;
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::node::NodeTraits;
use crate::dom::svg::svganimationelement::sample_targeted_numeric_animation;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{
    is_svg_whitespace, number_to_string, parse_number, parse_number_optional_number,
};

/// Which part of its attribute an animated number or integer reflects.
#[derive(Clone, Copy, Debug, JSTraceable, MallocSizeOf, PartialEq)]
pub(crate) enum SVGNumberPart {
    /// The whole attribute, which is a single number.
    Whole,
    /// The whole attribute, which is a number or a percentage, like the `offset` of a
    /// gradient stop.
    WholeOrPercentage,
    /// The first number of a `<number-optional-number>`.
    First,
    /// The second number of a `<number-optional-number>`, which defaults to the first.
    Second,
}

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedNumber>
#[dom_struct]
pub(crate) struct SVGAnimatedNumber {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    part: SVGNumberPart,
    /// The value when the attribute is missing or invalid.
    initial: f32,
}

impl SVGAnimatedNumber {
    pub(crate) fn new(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: LocalName,
        part: SVGNumberPart,
        initial: f32,
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

    /// Both numbers of a `<number-optional-number>` attribute.
    fn pair(&self) -> Option<(f32, f32)> {
        parse_number_optional_number(&self.element.svg_attribute(&self.attribute)?)
    }

    fn parse_value(&self, value: &str) -> Option<f32> {
        match self.part {
            SVGNumberPart::Whole => parse_number(value),
            SVGNumberPart::WholeOrPercentage => {
                let value = value.trim_matches(is_svg_whitespace);
                match value.strip_suffix('%') {
                    Some(percentage) => parse_number(percentage).map(|number| number / 100.0),
                    None => parse_number(value),
                }
            },
            SVGNumberPart::First => parse_number_optional_number(value).map(|(first, _)| first),
            SVGNumberPart::Second => parse_number_optional_number(value).map(|(_, second)| second),
        }
    }

    fn value(&self) -> f32 {
        self.element
            .svg_attribute(&self.attribute)
            .and_then(|value| self.parse_value(&value))
            .unwrap_or(self.initial)
    }
}

impl SVGAnimatedNumberMethods<crate::DomTypeHolder> for SVGAnimatedNumber {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedNumber__baseVal>
    fn BaseVal(&self) -> Finite<f32> {
        Finite::wrap(self.value())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedNumber__baseVal>
    fn SetBaseVal(&self, cx: &mut JSContext, value: Finite<f32>) {
        let value = *value;
        let string = match self.part {
            SVGNumberPart::Whole | SVGNumberPart::WholeOrPercentage => number_to_string(value),
            SVGNumberPart::First | SVGNumberPart::Second => {
                let (first, second) = self.pair().unwrap_or((self.initial, self.initial));
                let (first, second) = if self.part == SVGNumberPart::First {
                    (value, second)
                } else {
                    (first, value)
                };
                format!("{} {}", number_to_string(first), number_to_string(second))
            },
        };
        self.element.set_svg_attribute(cx, &self.attribute, string);
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedNumber__animVal>
    fn AnimVal(&self, cx: &mut JSContext) -> Finite<f32> {
        Finite::wrap(sample_targeted_numeric_animation(
            cx,
            &self.element,
            &self.attribute,
            self.value(),
            |value| self.parse_value(value),
        ))
    }
}
