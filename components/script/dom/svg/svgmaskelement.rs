/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGMaskElementBinding::SVGMaskElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{SVGLengthValue, svg_animated_enumeration, svg_animated_length};
use crate::dom::svg::svgunittypes::UNIT_TYPE_KEYWORDS;

/// <https://w3c.github.io/fxtf-drafts/css-masking-1/#InterfaceSVGMaskElement>
#[dom_struct]
pub(crate) struct SVGMaskElement {
    svgelement: SVGElement,
}

impl SVGMaskElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGMaskElement {
        SVGMaskElement {
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
    ) -> DomRoot<SVGMaskElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGMaskElement::new_inherited(local_name, prefix, document)),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGMaskElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGMaskElementMethods<crate::DomTypeHolder> for SVGMaskElement {
    svg_animated_enumeration!(MaskUnits, "maskUnits", "maskUnits", UNIT_TYPE_KEYWORDS, 2);
    svg_animated_enumeration!(
        MaskContentUnits,
        "maskContentUnits",
        "maskContentUnits",
        UNIT_TYPE_KEYWORDS,
        1
    );
    svg_animated_length!(X, "x", Horizontal, SVGLengthValue::percentage(-10.0));
    svg_animated_length!(Y, "y", Vertical, SVGLengthValue::percentage(-10.0));
    svg_animated_length!(
        Width,
        "width",
        Horizontal,
        SVGLengthValue::percentage(120.0)
    );
    svg_animated_length!(
        Height,
        "height",
        Vertical,
        SVGLengthValue::percentage(120.0)
    );
}
