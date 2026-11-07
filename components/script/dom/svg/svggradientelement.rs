/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGGradientElementBinding::SVGGradientElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::document::Document;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{
    svg_animated_enumeration, svg_animated_transform_list, svg_uri_reference,
};
use crate::dom::svg::svgunittypes::{SVG_UNIT_TYPE_OBJECTBOUNDINGBOX, UNIT_TYPE_KEYWORDS};

#[dom_struct]
pub(crate) struct SVGGradientElement {
    svgelement: SVGElement,
}

impl SVGGradientElement {
    pub(crate) fn new_inherited(
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGGradientElement {
        SVGGradientElement::new_inherited_with_state(
            ElementState::empty(),
            tag_name,
            prefix,
            document,
        )
    }

    pub(crate) fn new_inherited_with_state(
        state: ElementState,
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGGradientElement {
        SVGGradientElement {
            svgelement: SVGElement::new_inherited_with_state(state, tag_name, prefix, document),
        }
    }
}

impl VirtualMethods for SVGGradientElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGGradientElementMethods<crate::DomTypeHolder> for SVGGradientElement {
    svg_animated_transform_list!(GradientTransform, "gradientTransform", "gradientTransform");

    svg_animated_enumeration!(
        GradientUnits,
        "gradientUnits",
        "gradientUnits",
        UNIT_TYPE_KEYWORDS,
        SVG_UNIT_TYPE_OBJECTBOUNDINGBOX
    );
    svg_animated_enumeration!(
        SpreadMethod,
        "spreadMethod",
        "spreadMethod",
        &[("pad", 1), ("reflect", 2), ("repeat", 3)],
        1
    );

    // <https://svgwg.org/svg2-draft/types.html#__svg__SVGURIReference__href>
    svg_uri_reference!();
}
