/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name, ns};
use js::rust::HandleObject;
use style::attr::{AttrValue, parse_integer};
use style::values::CustomIdent;
use style::values::generics::counters::CounterPair;
use style::values::specified::{CounterReset, Integer};
use stylo_atoms::Atom;

use crate::dom::bindings::codegen::Bindings::HTMLOListElementBinding::HTMLOListElementMethods;
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
pub(crate) struct HTMLOListElement {
    htmlelement: HTMLElement,
}

impl HTMLOListElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> HTMLOListElement {
        HTMLOListElement {
            htmlelement: HTMLElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<HTMLOListElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(HTMLOListElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl LayoutDom<'_, HTMLOListElement> {
    /// The `counter-reset` presentational hint of the `start` and `reversed` attributes.
    ///
    /// <https://html.spec.whatwg.org/multipage/#lists>
    pub(crate) fn get_counter_reset(self) -> Option<CounterReset> {
        let element = self.upcast::<Element>();
        let start = element
            .get_attr_val_for_layout(&ns!(), &local_name!("start"))
            .and_then(|value| parse_integer(value.chars()).ok());
        let reversed = element
            .get_attr_for_layout(&ns!(), &local_name!("reversed"))
            .is_some();

        // `i32::MIN` is reserved for `reversed(list-item)` without a value, so clamp to the
        // range that `counter-reset` accepts.
        let value = match (reversed, start) {
            (true, Some(start)) => start.saturating_add(1),
            (true, None) => i32::MIN,
            (false, Some(start)) => start.saturating_sub(1).max(i32::MIN + 1),
            (false, None) => return None,
        };
        Some(CounterReset::new(vec![CounterPair {
            name: CustomIdent(Atom::from("list-item")),
            value: Integer::new(value),
            is_reversed: reversed,
        }]))
    }
}

impl HTMLOListElementMethods<crate::DomTypeHolder> for HTMLOListElement {
    // <https://html.spec.whatwg.org/multipage/#dom-ol-reversed>
    make_bool_getter!(Reversed, "reversed");

    // <https://html.spec.whatwg.org/multipage/#dom-ol-reversed>
    make_bool_setter!(SetReversed, "reversed");

    // <https://html.spec.whatwg.org/multipage/#dom-ol-start>
    make_int_getter!(Start, "start", 1);

    // <https://html.spec.whatwg.org/multipage/#dom-ol-start>
    make_int_setter!(SetStart, "start");

    // <https://html.spec.whatwg.org/multipage/#dom-ol-type>
    make_getter!(Type, "type");

    // <https://html.spec.whatwg.org/multipage/#dom-ol-type>
    make_setter!(SetType, "type");

    // <https://html.spec.whatwg.org/multipage/#dom-ol-compact>
    make_bool_getter!(Compact, "compact");

    // <https://html.spec.whatwg.org/multipage/#dom-ol-compact>
    make_bool_setter!(SetCompact, "compact");
}

impl VirtualMethods for HTMLOListElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<HTMLElement>() as &dyn VirtualMethods)
    }

    fn attribute_affects_presentational_hints(&self, attr: AttrRef<'_>) -> bool {
        match attr.local_name() {
            &local_name!("start") | &local_name!("reversed") => true,
            _ => self
                .super_type()
                .unwrap()
                .attribute_affects_presentational_hints(attr),
        }
    }

    fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue {
        match *name {
            local_name!("start") => AttrValue::from_i32(value.into(), 1),
            _ => self
                .super_type()
                .unwrap()
                .parse_plain_attribute(name, value),
        }
    }
}
