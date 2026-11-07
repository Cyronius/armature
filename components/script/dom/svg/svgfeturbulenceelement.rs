/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGFETurbulenceElementBinding::SVGFETurbulenceElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{
    svg_animated_enumeration, svg_animated_integer, svg_animated_number,
    svg_filter_primitive_standard_attributes,
};

/// <https://drafts.fxtf.org/filter-effects/#InterfaceSVGFETurbulenceElement>
#[dom_struct]
pub(crate) struct SVGFETurbulenceElement {
    svgelement: SVGElement,
}

impl SVGFETurbulenceElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGFETurbulenceElement {
        SVGFETurbulenceElement {
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
    ) -> DomRoot<SVGFETurbulenceElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGFETurbulenceElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGFETurbulenceElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGFETurbulenceElementMethods<crate::DomTypeHolder> for SVGFETurbulenceElement {
    svg_animated_number!(
        BaseFrequencyX,
        "baseFrequencyX",
        "baseFrequency",
        First,
        0.0
    );
    svg_animated_number!(
        BaseFrequencyY,
        "baseFrequencyY",
        "baseFrequency",
        Second,
        0.0
    );
    svg_animated_integer!(NumOctaves, "numOctaves", "numOctaves", Whole, 1);
    svg_animated_number!(Seed, "seed", "seed", Whole, 0.0);
    svg_animated_enumeration!(
        StitchTiles,
        "stitchTiles",
        "stitchTiles",
        &[("stitch", 1), ("noStitch", 2)],
        2
    );
    svg_animated_enumeration!(
        Type,
        "type",
        "type",
        &[("fractalNoise", 1), ("turbulence", 2)],
        2
    );

    // <https://drafts.fxtf.org/filter-effects/#InterfaceSVGFilterPrimitiveStandardAttributes>
    svg_filter_primitive_standard_attributes!();
}
