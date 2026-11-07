/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;

use crate::dom::bindings::codegen::Bindings::SVGSwitchElementBinding::SVGSwitchElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svggraphicselement::SVGGraphicsElement;

/// <https://svgwg.org/svg2-draft/struct.html#InterfaceSVGSwitchElement>
#[dom_struct]
pub(crate) struct SVGSwitchElement {
    svggraphicselement: SVGGraphicsElement,
}

impl SVGSwitchElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGSwitchElement {
        SVGSwitchElement {
            svggraphicselement: SVGGraphicsElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGSwitchElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGSwitchElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGSwitchElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGraphicsElement>() as &dyn VirtualMethods)
    }
}

impl SVGSwitchElementMethods<crate::DomTypeHolder> for SVGSwitchElement {}
