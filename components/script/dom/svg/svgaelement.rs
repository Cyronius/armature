/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name};
use js::rust::HandleObject;
use script_bindings::str::DOMString;
use style::attr::AttrValue;
use stylo_atoms::Atom;

use crate::dom::bindings::codegen::Bindings::SVGAElementBinding::SVGAElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::document::Document;
use crate::dom::domtokenlist::DOMTokenList;
use crate::dom::element::{Element, reflect_referrer_policy_attribute};
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svggraphicselement::SVGGraphicsElement;
use crate::dom::svg::svgtypes::{svg_animated_string, svg_uri_reference};

/// <https://svgwg.org/svg2-draft/linking.html#InterfaceSVGAElement>
#[dom_struct]
pub(crate) struct SVGAElement {
    svggraphicselement: SVGGraphicsElement,
    rel_list: MutNullableDom<DOMTokenList>,
}

impl SVGAElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGAElement {
        SVGAElement {
            svggraphicselement: SVGGraphicsElement::new_inherited(local_name, prefix, document),
            rel_list: Default::default(),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGAElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGAElement::new_inherited(local_name, prefix, document)),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGAElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGraphicsElement>() as &dyn VirtualMethods)
    }

    fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue {
        match name {
            &local_name!("rel") => AttrValue::from_serialized_tokenlist(value.into()),
            _ => self
                .super_type()
                .unwrap()
                .parse_plain_attribute(name, value),
        }
    }
}

impl SVGAElementMethods<crate::DomTypeHolder> for SVGAElement {
    svg_animated_string!(Target, "target", "target");

    // <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__download>
    make_getter!(Download, "download");

    // <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__download>
    make_setter!(SetDownload, "download");

    // <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__ping>
    make_usv_getter!(Ping, "ping");

    // <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__ping>
    make_usv_setter!(SetPing, "ping");

    // <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__rel>
    make_getter!(Rel, "rel");

    /// <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__rel>
    fn SetRel(&self, cx: &mut js::context::JSContext, rel: DOMString) {
        self.upcast::<Element>()
            .set_tokenlist_attribute(cx, &local_name!("rel"), rel);
    }

    /// <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__relList>
    fn RelList(&self, cx: &mut js::context::JSContext) -> DomRoot<DOMTokenList> {
        self.rel_list.or_init(|| {
            DOMTokenList::new(
                cx,
                self.upcast(),
                &local_name!("rel"),
                Some(vec![
                    Atom::from("noopener"),
                    Atom::from("noreferrer"),
                    Atom::from("opener"),
                ]),
            )
        })
    }

    // <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__hreflang>
    make_getter!(Hreflang, "hreflang");

    // <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__hreflang>
    make_setter!(SetHreflang, "hreflang");

    // <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__type>
    make_getter!(Type, "type");

    // <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__type>
    make_setter!(SetType, "type");

    /// <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__referrerPolicy>
    fn ReferrerPolicy(&self) -> DOMString {
        reflect_referrer_policy_attribute(self.upcast::<Element>())
    }

    // <https://svgwg.org/svg2-draft/linking.html#__svg__SVGAElement__referrerPolicy>
    make_setter!(SetReferrerPolicy, "referrerpolicy");

    // <https://svgwg.org/svg2-draft/types.html#__svg__SVGURIReference__href>
    svg_uri_reference!();
}
