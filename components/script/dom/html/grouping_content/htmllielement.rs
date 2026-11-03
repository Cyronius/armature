/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name, ns};
use js::rust::HandleObject;
use style::attr::{AttrValue, parse_integer};
use style::values::CustomIdent;
use style::values::generics::counters::CounterPair;
use style::values::specified::{CounterSet, Integer};
use stylo_atoms::Atom;

use crate::dom::bindings::codegen::Bindings::HTMLLIElementBinding::HTMLLIElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{DomRoot, LayoutDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::element::Element;
use crate::dom::element::attributes::storage::AttrRef;
use crate::dom::html::htmlelement::HTMLElement;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;

#[dom_struct]
pub(crate) struct HTMLLIElement {
    htmlelement: HTMLElement,
}

impl HTMLLIElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> HTMLLIElement {
        HTMLLIElement {
            htmlelement: HTMLElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<HTMLLIElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(HTMLLIElement::new_inherited(local_name, prefix, document)),
            document,
            proto,
        )
    }
}

impl LayoutDom<'_, HTMLLIElement> {
    /// The `counter-set` presentational hint of the `value` attribute.
    ///
    /// <https://html.spec.whatwg.org/multipage/#lists>
    pub(crate) fn get_counter_set(self) -> Option<CounterSet> {
        let value = self
            .upcast::<Element>()
            .get_attr_val_for_layout(&ns!(), &local_name!("value"))
            .and_then(|value| parse_integer(value.chars()).ok())?;

        // `i32::MIN` is reserved by Stylo, so clamp to the range that `counter-set` accepts.
        Some(CounterSet::new(vec![CounterPair {
            name: CustomIdent(Atom::from("list-item")),
            value: Integer::new(value.max(i32::MIN + 1)),
            is_reversed: false,
        }]))
    }
}

impl HTMLLIElementMethods<crate::DomTypeHolder> for HTMLLIElement {
    // <https://html.spec.whatwg.org/multipage/#dom-li-type>
    make_getter!(Type, "type");

    // <https://html.spec.whatwg.org/multipage/#dom-li-type>
    make_setter!(SetType, "type");

    // https://html.spec.whatwg.org/multipage/#dom-li-value
    make_int_getter!(Value, "value");

    // https://html.spec.whatwg.org/multipage/#dom-li-value
    make_int_setter!(SetValue, "value");
}

impl VirtualMethods for HTMLLIElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<HTMLElement>() as &dyn VirtualMethods)
    }

    fn attribute_affects_presentational_hints(&self, attr: AttrRef<'_>) -> bool {
        match attr.local_name() {
            &local_name!("value") => true,
            _ => self
                .super_type()
                .unwrap()
                .attribute_affects_presentational_hints(attr),
        }
    }

    fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue {
        match name {
            &local_name!("value") => AttrValue::from_i32(value.into(), 0),
            _ => self
                .super_type()
                .unwrap()
                .parse_plain_attribute(name, value),
        }
    }
}
