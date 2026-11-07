/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGFEBlendElementBinding::SVGFEBlendElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{
    svg_animated_enumeration, svg_animated_string, svg_filter_primitive_standard_attributes,
};

/// <https://drafts.fxtf.org/filter-effects/#InterfaceSVGFEBlendElement>
#[dom_struct]
pub(crate) struct SVGFEBlendElement {
    svgelement: SVGElement,
}

impl SVGFEBlendElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGFEBlendElement {
        SVGFEBlendElement {
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
    ) -> DomRoot<SVGFEBlendElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGFEBlendElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGFEBlendElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGFEBlendElementMethods<crate::DomTypeHolder> for SVGFEBlendElement {
    svg_animated_string!(In1, "in1", "in");
    svg_animated_string!(In2, "in2", "in2");
    svg_animated_enumeration!(
        Mode,
        "mode",
        "mode",
        &[
            ("normal", 1),
            ("multiply", 2),
            ("screen", 3),
            ("darken", 4),
            ("lighten", 5),
            ("overlay", 6),
            ("color-dodge", 7),
            ("color-burn", 8),
            ("hard-light", 9),
            ("soft-light", 10),
            ("difference", 11),
            ("exclusion", 12),
            ("hue", 13),
            ("saturation", 14),
            ("color", 15),
            ("luminosity", 16)
        ],
        1
    );

    // <https://drafts.fxtf.org/filter-effects/#InterfaceSVGFilterPrimitiveStandardAttributes>
    svg_filter_primitive_standard_attributes!();
}
