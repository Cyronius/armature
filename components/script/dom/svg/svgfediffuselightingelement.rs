/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGFEDiffuseLightingElementBinding::SVGFEDiffuseLightingElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{
    svg_animated_number, svg_animated_string, svg_filter_primitive_standard_attributes,
};

/// <https://w3c.github.io/fxtf-drafts/filter-effects-1/#InterfaceSVGFEDiffuseLightingElement>
#[dom_struct]
pub(crate) struct SVGFEDiffuseLightingElement {
    svgelement: SVGElement,
}

impl SVGFEDiffuseLightingElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGFEDiffuseLightingElement {
        SVGFEDiffuseLightingElement {
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
    ) -> DomRoot<SVGFEDiffuseLightingElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGFEDiffuseLightingElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGFEDiffuseLightingElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGFEDiffuseLightingElementMethods<crate::DomTypeHolder> for SVGFEDiffuseLightingElement {
    svg_animated_string!(In1, "in1", "in");
    svg_animated_number!(SurfaceScale, "surfaceScale", "surfaceScale", Whole, 1.0);
    svg_animated_number!(
        DiffuseConstant,
        "diffuseConstant",
        "diffuseConstant",
        Whole,
        1.0
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

    // <https://w3c.github.io/fxtf-drafts/filter-effects-1/#InterfaceSVGFilterPrimitiveStandardAttributes>
    svg_filter_primitive_standard_attributes!();
}
