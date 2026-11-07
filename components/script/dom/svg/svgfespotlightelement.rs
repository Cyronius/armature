/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGFESpotLightElementBinding::SVGFESpotLightElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::svg_animated_number;

/// <https://drafts.fxtf.org/filter-effects/#InterfaceSVGFESpotLightElement>
#[dom_struct]
pub(crate) struct SVGFESpotLightElement {
    svgelement: SVGElement,
}

impl SVGFESpotLightElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGFESpotLightElement {
        SVGFESpotLightElement {
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
    ) -> DomRoot<SVGFESpotLightElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGFESpotLightElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGFESpotLightElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGFESpotLightElementMethods<crate::DomTypeHolder> for SVGFESpotLightElement {
    svg_animated_number!(X, "x", "x", Whole, 0.0);
    svg_animated_number!(Y, "y", "y", Whole, 0.0);
    svg_animated_number!(Z, "z", "z", Whole, 0.0);
    svg_animated_number!(PointsAtX, "pointsAtX", "pointsAtX", Whole, 0.0);
    svg_animated_number!(PointsAtY, "pointsAtY", "pointsAtY", Whole, 0.0);
    svg_animated_number!(PointsAtZ, "pointsAtZ", "pointsAtZ", Whole, 0.0);
    svg_animated_number!(
        SpecularExponent,
        "specularExponent",
        "specularExponent",
        Whole,
        1.0
    );
    svg_animated_number!(
        LimitingConeAngle,
        "limitingConeAngle",
        "limitingConeAngle",
        Whole,
        0.0
    );
}
