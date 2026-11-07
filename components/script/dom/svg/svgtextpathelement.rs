/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;

use crate::dom::bindings::codegen::Bindings::SVGTextPathElementBinding::SVGTextPathElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgtextcontentelement::SVGTextContentElement;
use crate::dom::svg::svgtypes::{
    SVGLengthValue, svg_animated_enumeration, svg_animated_length, svg_uri_reference,
};

/// <https://svgwg.org/svg2-draft/text.html#InterfaceSVGTextPathElement>
#[dom_struct]
pub(crate) struct SVGTextPathElement {
    svgtextcontentelement: SVGTextContentElement,
}

impl SVGTextPathElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGTextPathElement {
        SVGTextPathElement {
            svgtextcontentelement: SVGTextContentElement::new_inherited(
                local_name, prefix, document,
            ),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGTextPathElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGTextPathElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGTextPathElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGTextContentElement>() as &dyn VirtualMethods)
    }
}

impl SVGTextPathElementMethods<crate::DomTypeHolder> for SVGTextPathElement {
    svg_animated_length!(
        StartOffset,
        "startOffset",
        Horizontal,
        SVGLengthValue::number(0.0)
    );
    svg_animated_enumeration!(
        Method,
        "method",
        "method",
        &[("align", 1), ("stretch", 2)],
        1
    );
    svg_animated_enumeration!(
        Spacing,
        "spacing",
        "spacing",
        &[("auto", 1), ("exact", 2)],
        2
    );
    svg_animated_enumeration!(Side, "side", "side", &[("left", 1), ("right", 2)], 1);

    // <https://svgwg.org/svg2-draft/types.html#__svg__SVGURIReference__href>
    svg_uri_reference!();
}
