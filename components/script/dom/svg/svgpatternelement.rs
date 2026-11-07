/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGPatternElementBinding::SVGPatternElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{
    SVGLengthValue, svg_animated_enumeration, svg_animated_length, svg_animated_transform_list,
    svg_fit_to_view_box, svg_uri_reference,
};
use crate::dom::svg::svgunittypes::{
    SVG_UNIT_TYPE_OBJECTBOUNDINGBOX, SVG_UNIT_TYPE_USERSPACEONUSE, UNIT_TYPE_KEYWORDS,
};

#[dom_struct]
pub(crate) struct SVGPatternElement {
    svgelement: SVGElement,
}

impl SVGPatternElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGPatternElement {
        SVGPatternElement {
            svgelement: SVGElement::new_inherited_with_state(
                ElementState::empty(),
                local_name,
                prefix,
                document,
            ),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGPatternElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGPatternElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGPatternElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGPatternElementMethods<crate::DomTypeHolder> for SVGPatternElement {
    svg_animated_transform_list!(PatternTransform, "patternTransform", "patternTransform");

    svg_fit_to_view_box!();

    svg_animated_enumeration!(
        PatternUnits,
        "patternUnits",
        "patternUnits",
        UNIT_TYPE_KEYWORDS,
        SVG_UNIT_TYPE_OBJECTBOUNDINGBOX
    );
    svg_animated_enumeration!(
        PatternContentUnits,
        "patternContentUnits",
        "patternContentUnits",
        UNIT_TYPE_KEYWORDS,
        SVG_UNIT_TYPE_USERSPACEONUSE
    );
    svg_animated_length!(X, "x", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Y, "y", Vertical, SVGLengthValue::number(0.0));
    svg_animated_length!(Width, "width", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Height, "height", Vertical, SVGLengthValue::number(0.0));

    // <https://svgwg.org/svg2-draft/types.html#__svg__SVGURIReference__href>
    svg_uri_reference!();
}
