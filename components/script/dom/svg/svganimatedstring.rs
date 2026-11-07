/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, local_name, ns};
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};
use script_bindings::str::DOMString;

use crate::dom::bindings::codegen::Bindings::SVGAnimatedStringBinding::SVGAnimatedStringMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::element::Element;
use crate::dom::node::NodeTraits;
use crate::dom::svg::svgelement::SVGElement;

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedString>
#[dom_struct]
pub(crate) struct SVGAnimatedString {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
}

impl SVGAnimatedString {
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

    /// Whether this reflects `href`, which falls back to `xlink:href`.
    ///
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGURIReference__href>
    fn is_href(&self) -> bool {
        self.attribute == local_name!("href")
    }

    fn value(&self) -> DOMString {
        let element = self.element.upcast::<Element>();
        let value = element
            .get_attribute_string_value_with_namespace(&ns!(), &self.attribute)
            .or_else(|| {
                self.is_href()
                    .then(|| {
                        element.get_attribute_string_value_with_namespace(
                            &ns!(xlink),
                            &local_name!("href"),
                        )
                    })
                    .flatten()
            });
        value.map(DOMString::from).unwrap_or_default()
    }
}

impl SVGAnimatedStringMethods<crate::DomTypeHolder> for SVGAnimatedString {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedString__baseVal>
    fn BaseVal(&self) -> DOMString {
        self.value()
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedString__baseVal>
    fn SetBaseVal(&self, cx: &mut JSContext, value: DOMString) {
        let element = self.element.upcast::<Element>();
        // > If the reflected attribute is href and it is not present but xlink:href is, set
        // > xlink:href instead.
        if self.is_href() &&
            !element.has_attribute(&local_name!("href")) &&
            element
                .get_attribute_string_value_with_namespace(&ns!(xlink), &local_name!("href"))
                .is_some()
        {
            let value = element.parse_attribute(&ns!(xlink), &local_name!("href"), value);
            element.set_attribute_with_namespace(
                cx,
                local_name!("href"),
                value,
                LocalName::from("xlink:href"),
                ns!(xlink),
                Some(html5ever::Prefix::from("xlink")),
            );
            return;
        }
        self.element
            .set_svg_attribute(cx, &self.attribute, String::from(value.str().as_ref()));
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedString__animVal>
    fn AnimVal(&self) -> DOMString {
        self.value()
    }
}
