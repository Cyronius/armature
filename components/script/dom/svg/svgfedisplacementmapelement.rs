/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGFEDisplacementMapElementBinding::SVGFEDisplacementMapElementMethods;
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

/// <https://w3c.github.io/fxtf-drafts/filter-effects-1/#InterfaceSVGFEDisplacementMapElement>
#[dom_struct]
pub(crate) struct SVGFEDisplacementMapElement {
    svgelement: SVGElement,
}

impl SVGFEDisplacementMapElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGFEDisplacementMapElement {
        SVGFEDisplacementMapElement {
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
    ) -> DomRoot<SVGFEDisplacementMapElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGFEDisplacementMapElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGFEDisplacementMapElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGFEDisplacementMapElementMethods<crate::DomTypeHolder> for SVGFEDisplacementMapElement {
    svg_animated_string!(In1, "in1", "in");
    svg_animated_string!(In2, "in2", "in2");
    svg_animated_number!(Scale, "scale", "scale", Whole, 0.0);
    svg_animated_enumeration!(
        XChannelSelector,
        "xChannelSelector",
        "xChannelSelector",
        &[("R", 1), ("G", 2), ("B", 3), ("A", 4)],
        4
    );
    svg_animated_enumeration!(
        YChannelSelector,
        "yChannelSelector",
        "yChannelSelector",
        &[("R", 1), ("G", 2), ("B", 3), ("A", 4)],
        4
    );

    // <https://w3c.github.io/fxtf-drafts/filter-effects-1/#InterfaceSVGFilterPrimitiveStandardAttributes>
    svg_filter_primitive_standard_attributes!();
}
