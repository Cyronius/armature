/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, local_name};
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedBooleanBinding::SVGAnimatedBooleanMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::node::{Node, NodeTraits};
use crate::dom::svg::svganimationelement::{SVGAnimationElement, trim_one_trailing_semicolon};
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

    fn animated_value(&self) -> bool {
        fn parse(value: &str) -> Option<bool> {
            match value.trim_matches(is_svg_whitespace) {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            }
        }
        let mut result = self.value();
        for child in self.element.upcast::<Node>().child_elements() {
            if child.local_name() != &local_name!("animate") &&
                child.local_name() != &local_name!("set")
            {
                continue;
            }
            let Some(animation) = child.downcast::<SVGAnimationElement>() else {
                continue;
            };
            let element = animation.upcast::<SVGElement>();
            if element
                .svg_attribute(&local_name!("attributeName"))
                .as_deref() !=
                Some(self.attribute.as_ref())
            {
                continue;
            }
            if child.local_name() == &local_name!("set") {
                if animation.set_is_active() &&
                    let Some(to) = element
                        .svg_attribute(&local_name!("to"))
                        .and_then(|value| parse(&value))
                {
                    result = to;
                }
                continue;
            }
            if let Some(values) = element.svg_attribute(&local_name!("values")) {
                let values: Option<Vec<_>> = trim_one_trailing_semicolon(&values)
                    .split(';')
                    .map(parse)
                    .collect();
                if let Some(values) = values &&
                    let Some(index) = animation.sample_discrete_index(values.len())
                {
                    result = values[index];
                }
                continue;
            }
            if let Some(to) = element
                .svg_attribute(&local_name!("to"))
                .and_then(|value| parse(&value))
            {
                if let Some(index) = animation.sample_discrete_index(2) {
                    result = if index == 0 || animation.sample_progress() == Some(0.5) {
                        element
                            .svg_attribute(&local_name!("from"))
                            .as_deref()
                            .and_then(parse)
                            .unwrap_or(result)
                    } else {
                        to
                    };
                }
            }
        }
        result
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
        self.animated_value()
    }
}
