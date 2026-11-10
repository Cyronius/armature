/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGFECompositeElementBinding::SVGFECompositeElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{
    svg_animated_enumeration, svg_animated_number, svg_animated_string,
    svg_filter_primitive_standard_attributes,
};

/// <https://w3c.github.io/fxtf-drafts/filter-effects-1/#InterfaceSVGFECompositeElement>
#[dom_struct]
pub(crate) struct SVGFECompositeElement {
    svgelement: SVGElement,
}

impl SVGFECompositeElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGFECompositeElement {
        SVGFECompositeElement {
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
    ) -> DomRoot<SVGFECompositeElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGFECompositeElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGFECompositeElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGFECompositeElementMethods<crate::DomTypeHolder> for SVGFECompositeElement {
    svg_animated_string!(In1, "in1", "in");
    svg_animated_string!(In2, "in2", "in2");
    svg_animated_enumeration!(
        Operator,
        "operator",
        "operator",
        &[
            ("over", 1),
            ("in", 2),
            ("out", 3),
            ("atop", 4),
            ("xor", 5),
            ("arithmetic", 6),
            ("lighter", 7)
        ],
        1
    );
    svg_animated_number!(K1, "k1", "k1", Whole, 0.0);
    svg_animated_number!(K2, "k2", "k2", Whole, 0.0);
    svg_animated_number!(K3, "k3", "k3", Whole, 0.0);
    svg_animated_number!(K4, "k4", "k4", Whole, 0.0);

    // <https://w3c.github.io/fxtf-drafts/filter-effects-1/#InterfaceSVGFilterPrimitiveStandardAttributes>
    svg_filter_primitive_standard_attributes!();
}
