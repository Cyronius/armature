/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;

use crate::dom::bindings::codegen::Bindings::SVGRectElementBinding::SVGRectElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svggeometryelement::SVGGeometryElement;
use crate::dom::svg::svgtypes::{SVGLengthValue, svg_animated_length};

#[dom_struct]
pub(crate) struct SVGRectElement {
    svggeometryelement: SVGGeometryElement,
}

impl SVGRectElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGRectElement {
        SVGRectElement {
            svggeometryelement: SVGGeometryElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGRectElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGRectElement::new_inherited(local_name, prefix, document)),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGRectElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGeometryElement>() as &dyn VirtualMethods)
    }
}

impl SVGRectElementMethods<crate::DomTypeHolder> for SVGRectElement {
    svg_animated_length!(X, "x", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Y, "y", Vertical, SVGLengthValue::number(0.0));
    svg_animated_length!(Width, "width", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Height, "height", Vertical, SVGLengthValue::number(0.0));
    svg_animated_length!(Rx, "rx", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Ry, "ry", Vertical, SVGLengthValue::number(0.0));
}
