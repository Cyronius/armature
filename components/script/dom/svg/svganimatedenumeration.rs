/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::LocalName;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedEnumerationBinding::SVGAnimatedEnumerationMethods;
use crate::dom::bindings::error::{Error, ErrorResult};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::node::NodeTraits;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::is_svg_whitespace;

/// The keywords of an enumerated attribute with the values of their enumeration constants.
pub(crate) type SVGEnumerationKeywords = &'static [(&'static str, u16)];

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedEnumeration>
#[dom_struct]
pub(crate) struct SVGAnimatedEnumeration {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    #[no_trace]
    #[ignore_malloc_size_of = "static"]
    keywords: SVGEnumerationKeywords,
    /// The value when the attribute is missing or invalid.
    initial: u16,
}

impl SVGAnimatedEnumeration {
    pub(crate) fn new(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: LocalName,
        keywords: SVGEnumerationKeywords,
        initial: u16,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                element: Dom::from_ref(element),
                attribute,
                keywords,
                initial,
            }),
            &*element.owner_window(),
        )
    }

    fn value(&self) -> u16 {
        // > Enumerated attributes revert to their initial value when the attribute value
        // > does not match the grammar in the attribute definition table.
        self.element
            .svg_attribute(&self.attribute)
            .and_then(|value| {
                let value = value.trim_matches(is_svg_whitespace);
                self.keywords
                    .iter()
                    .find(|(keyword, _)| *keyword == value)
                    .map(|(_, constant)| *constant)
            })
            .unwrap_or(self.initial)
    }
}

impl SVGAnimatedEnumerationMethods<crate::DomTypeHolder> for SVGAnimatedEnumeration {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedEnumeration__baseVal>
    fn BaseVal(&self) -> u16 {
        self.value()
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedEnumeration__baseVal>
    fn SetBaseVal(&self, cx: &mut JSContext, value: u16) -> ErrorResult {
        // > If value is 0 or is not the numeric type value for any value of the reflected
        // > attribute, then a TypeError is thrown.
        let Some((keyword, _)) = self
            .keywords
            .iter()
            .find(|(_, constant)| value != 0 && *constant == value)
        else {
            return Err(Error::Type(c"Invalid enumeration value".to_owned()));
        };
        self.element
            .set_svg_attribute(cx, &self.attribute, keyword.to_string());
        Ok(())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedEnumeration__animVal>
    fn AnimVal(&self) -> u16 {
        self.value()
    }
}
