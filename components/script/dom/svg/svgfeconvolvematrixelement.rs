/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGFEConvolveMatrixElementBinding::SVGFEConvolveMatrixElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{
    svg_animated_boolean, svg_animated_enumeration, svg_animated_integer, svg_animated_number,
    svg_animated_number_list, svg_animated_string, svg_filter_primitive_standard_attributes,
};

/// <https://drafts.fxtf.org/filter-effects/#InterfaceSVGFEConvolveMatrixElement>
#[dom_struct]
pub(crate) struct SVGFEConvolveMatrixElement {
    svgelement: SVGElement,
}

impl SVGFEConvolveMatrixElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGFEConvolveMatrixElement {
        SVGFEConvolveMatrixElement {
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
    ) -> DomRoot<SVGFEConvolveMatrixElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGFEConvolveMatrixElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGFEConvolveMatrixElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGFEConvolveMatrixElementMethods<crate::DomTypeHolder> for SVGFEConvolveMatrixElement {
    svg_animated_number_list!(KernelMatrix, "kernelMatrix", "kernelMatrix");

    svg_animated_string!(In1, "in1", "in");
    svg_animated_integer!(OrderX, "orderX", "order", First, 3);
    svg_animated_integer!(OrderY, "orderY", "order", Second, 3);
    svg_animated_number!(Divisor, "divisor", "divisor", Whole, 1.0);
    svg_animated_number!(Bias, "bias", "bias", Whole, 0.0);
    svg_animated_integer!(TargetX, "targetX", "targetX", Whole, 0);
    svg_animated_integer!(TargetY, "targetY", "targetY", Whole, 0);
    svg_animated_enumeration!(
        EdgeMode,
        "edgeMode",
        "edgeMode",
        &[("duplicate", 1), ("wrap", 2), ("none", 3)],
        1
    );
    svg_animated_number!(
        KernelUnitLengthX,
        "kernelUnitLengthX",
        "kernelUnitLength",
        First,
        0.0
    );
    svg_animated_number!(
        KernelUnitLengthY,
        "kernelUnitLengthY",
        "kernelUnitLength",
        Second,
        0.0
    );
    svg_animated_boolean!(PreserveAlpha, "preserveAlpha", "preserveAlpha");

    // <https://drafts.fxtf.org/filter-effects/#InterfaceSVGFilterPrimitiveStandardAttributes>
    svg_filter_primitive_standard_attributes!();
}
