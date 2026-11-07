/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;

use crate::dom::bindings::codegen::Bindings::SVGForeignObjectElementBinding::SVGForeignObjectElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svggraphicselement::SVGGraphicsElement;
use crate::dom::svg::svgtypes::{SVGLengthValue, svg_animated_length};

/// <https://svgwg.org/svg2-draft/embedded.html#InterfaceSVGForeignObjectElement>
#[dom_struct]
pub(crate) struct SVGForeignObjectElement {
    svggraphicselement: SVGGraphicsElement,
}

impl SVGForeignObjectElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGForeignObjectElement {
        SVGForeignObjectElement {
            svggraphicselement: SVGGraphicsElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGForeignObjectElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGForeignObjectElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGForeignObjectElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGraphicsElement>() as &dyn VirtualMethods)
    }
}

impl SVGForeignObjectElementMethods<crate::DomTypeHolder> for SVGForeignObjectElement {
    svg_animated_length!(X, "x", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Y, "y", Vertical, SVGLengthValue::number(0.0));
    svg_animated_length!(Width, "width", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Height, "height", Vertical, SVGLengthValue::number(0.0));
}
