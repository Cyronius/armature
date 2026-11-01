/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;

use crate::dom::bindings::codegen::Bindings::HTMLParamElementBinding::HTMLParamElementMethods;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::html::htmlelement::HTMLElement;
use crate::dom::node::Node;

#[dom_struct]
pub(crate) struct HTMLParamElement {
    htmlelement: HTMLElement,
}

impl HTMLParamElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> HTMLParamElement {
        HTMLParamElement {
            htmlelement: HTMLElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<HTMLParamElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(HTMLParamElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl HTMLParamElementMethods<crate::DomTypeHolder> for HTMLParamElement {
    // <https://html.spec.whatwg.org/multipage/#dom-param-name>
    make_getter!(Name, "name");

    // <https://html.spec.whatwg.org/multipage/#dom-param-name>
    make_atomic_setter!(SetName, "name");

    // <https://html.spec.whatwg.org/multipage/#dom-param-value>
    make_getter!(Value, "value");

    // <https://html.spec.whatwg.org/multipage/#dom-param-value>
    make_setter!(SetValue, "value");

    // <https://html.spec.whatwg.org/multipage/#dom-param-type>
    make_getter!(Type, "type");

    // <https://html.spec.whatwg.org/multipage/#dom-param-type>
    make_setter!(SetType, "type");

    // <https://html.spec.whatwg.org/multipage/#dom-param-valuetype>
    make_getter!(ValueType, "valuetype");

    // <https://html.spec.whatwg.org/multipage/#dom-param-valuetype>
    make_setter!(SetValueType, "valuetype");
}
