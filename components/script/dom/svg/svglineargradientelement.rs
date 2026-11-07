/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;

use crate::dom::bindings::codegen::Bindings::SVGLinearGradientElementBinding::SVGLinearGradientElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svggradientelement::SVGGradientElement;
use crate::dom::svg::svgtypes::{SVGLengthValue, svg_animated_length};

#[dom_struct]
pub(crate) struct SVGLinearGradientElement {
    svggradientelement: SVGGradientElement,
}

impl SVGLinearGradientElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGLinearGradientElement {
        SVGLinearGradientElement {
            svggradientelement: SVGGradientElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGLinearGradientElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGLinearGradientElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGLinearGradientElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGradientElement>() as &dyn VirtualMethods)
    }
}

impl SVGLinearGradientElementMethods<crate::DomTypeHolder> for SVGLinearGradientElement {
    svg_animated_length!(X1, "x1", Horizontal, SVGLengthValue::percentage(0.0));
    svg_animated_length!(Y1, "y1", Vertical, SVGLengthValue::percentage(0.0));
    svg_animated_length!(X2, "x2", Horizontal, SVGLengthValue::percentage(100.0));
    svg_animated_length!(Y2, "y2", Vertical, SVGLengthValue::percentage(0.0));
}
