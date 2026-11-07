/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGFEDropShadowElementBinding::SVGFEDropShadowElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{
    svg_animated_number, svg_animated_string, svg_filter_primitive_standard_attributes,
    svg_set_std_deviation,
};

/// <https://drafts.fxtf.org/filter-effects/#InterfaceSVGFEDropShadowElement>
#[dom_struct]
pub(crate) struct SVGFEDropShadowElement {
    svgelement: SVGElement,
}

impl SVGFEDropShadowElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGFEDropShadowElement {
        SVGFEDropShadowElement {
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
    ) -> DomRoot<SVGFEDropShadowElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGFEDropShadowElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGFEDropShadowElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGFEDropShadowElementMethods<crate::DomTypeHolder> for SVGFEDropShadowElement {
    svg_animated_string!(In1, "in1", "in");
    svg_animated_number!(Dx, "dx", "dx", Whole, 2.0);
    svg_animated_number!(Dy, "dy", "dy", Whole, 2.0);
    svg_animated_number!(StdDeviationX, "stdDeviationX", "stdDeviation", First, 2.0);
    svg_animated_number!(StdDeviationY, "stdDeviationY", "stdDeviation", Second, 2.0);

    // <https://drafts.fxtf.org/filter-effects/#InterfaceSVGFilterPrimitiveStandardAttributes>
    svg_filter_primitive_standard_attributes!();
    svg_set_std_deviation!();
}
