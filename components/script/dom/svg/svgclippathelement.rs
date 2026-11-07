/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGClipPathElementBinding::SVGClipPathElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{svg_animated_enumeration, svg_animated_transform_list};
use crate::dom::svg::svgunittypes::UNIT_TYPE_KEYWORDS;

/// <https://w3c.github.io/fxtf-drafts/css-masking-1/#InterfaceSVGClipPathElement>
#[dom_struct]
pub(crate) struct SVGClipPathElement {
    svgelement: SVGElement,
}

impl SVGClipPathElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGClipPathElement {
        SVGClipPathElement {
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
    ) -> DomRoot<SVGClipPathElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGClipPathElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGClipPathElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGClipPathElementMethods<crate::DomTypeHolder> for SVGClipPathElement {
    svg_animated_transform_list!(Transform, "transform", "transform");

    svg_animated_enumeration!(
        ClipPathUnits,
        "clipPathUnits",
        "clipPathUnits",
        UNIT_TYPE_KEYWORDS,
        1
    );
}
