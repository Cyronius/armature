/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGFEPointLightElementBinding::SVGFEPointLightElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::svg_animated_number;

/// <https://w3c.github.io/fxtf-drafts/filter-effects-1/#InterfaceSVGFEPointLightElement>
#[dom_struct]
pub(crate) struct SVGFEPointLightElement {
    svgelement: SVGElement,
}

impl SVGFEPointLightElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGFEPointLightElement {
        SVGFEPointLightElement {
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
    ) -> DomRoot<SVGFEPointLightElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGFEPointLightElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGFEPointLightElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGFEPointLightElementMethods<crate::DomTypeHolder> for SVGFEPointLightElement {
    svg_animated_number!(X, "x", "x", Whole, 0.0);
    svg_animated_number!(Y, "y", "y", Whole, 0.0);
    svg_animated_number!(Z, "z", "z", Whole, 0.0);
}
