/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGFESpecularLightingElementBinding::SVGFESpecularLightingElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{
    svg_animated_number, svg_animated_string, svg_filter_primitive_standard_attributes,
};

/// <https://w3c.github.io/fxtf-drafts/filter-effects-1/#InterfaceSVGFESpecularLightingElement>
#[dom_struct]
pub(crate) struct SVGFESpecularLightingElement {
    svgelement: SVGElement,
}

impl SVGFESpecularLightingElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGFESpecularLightingElement {
        SVGFESpecularLightingElement {
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
    ) -> DomRoot<SVGFESpecularLightingElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGFESpecularLightingElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGFESpecularLightingElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGFESpecularLightingElementMethods<crate::DomTypeHolder> for SVGFESpecularLightingElement {
    svg_animated_string!(In1, "in1", "in");
    svg_animated_number!(SurfaceScale, "surfaceScale", "surfaceScale", Whole, 1.0);
    svg_animated_number!(
        SpecularConstant,
        "specularConstant",
        "specularConstant",
        Whole,
        1.0
    );
    svg_animated_number!(
        SpecularExponent,
        "specularExponent",
        "specularExponent",
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
