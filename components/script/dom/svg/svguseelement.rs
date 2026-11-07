/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;

use crate::dom::bindings::codegen::Bindings::SVGUseElementBinding::SVGUseElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svggraphicselement::SVGGraphicsElement;
use crate::dom::svg::svgtypes::{SVGLengthValue, svg_animated_length, svg_uri_reference};

#[dom_struct]
pub(crate) struct SVGUseElement {
    svggraphicselement: SVGGraphicsElement,
}

impl SVGUseElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGUseElement {
        SVGUseElement {
            svggraphicselement: SVGGraphicsElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGUseElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGUseElement::new_inherited(local_name, prefix, document)),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGUseElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGraphicsElement>() as &dyn VirtualMethods)
    }
}

impl SVGUseElementMethods<crate::DomTypeHolder> for SVGUseElement {
    svg_animated_length!(X, "x", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Y, "y", Vertical, SVGLengthValue::number(0.0));
    svg_animated_length!(Width, "width", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Height, "height", Vertical, SVGLengthValue::number(0.0));

    // <https://svgwg.org/svg2-draft/types.html#__svg__SVGURIReference__href>
    svg_uri_reference!();
}
