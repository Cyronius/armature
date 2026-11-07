/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;

use crate::dom::bindings::codegen::Bindings::SVGLineElementBinding::SVGLineElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svggeometryelement::SVGGeometryElement;
use crate::dom::svg::svgtypes::{SVGLengthValue, svg_animated_length};

#[dom_struct]
pub(crate) struct SVGLineElement {
    svggeometryelement: SVGGeometryElement,
}

impl SVGLineElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGLineElement {
        SVGLineElement {
            svggeometryelement: SVGGeometryElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGLineElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGLineElement::new_inherited(local_name, prefix, document)),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGLineElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGeometryElement>() as &dyn VirtualMethods)
    }
}

impl SVGLineElementMethods<crate::DomTypeHolder> for SVGLineElement {
    svg_animated_length!(X1, "x1", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Y1, "y1", Vertical, SVGLengthValue::number(0.0));
    svg_animated_length!(X2, "x2", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Y2, "y2", Vertical, SVGLengthValue::number(0.0));
}
