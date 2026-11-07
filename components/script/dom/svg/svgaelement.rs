/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name};
use js::context::NoGC;
use js::rust::HandleObject;
use net_traits::blob_url_store::UrlWithBlobClaim;
use script_bindings::cell::DomRefCell;
use script_bindings::str::{DOMString, USVString};
use style::attr::AttrValue;
use stylo_atoms::Atom;

use crate::dom::bindings::codegen::Bindings::SVGAElementBinding::SVGAElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::document::Document;
use crate::dom::domtokenlist::DOMTokenList;
use crate::dom::element::{Element, reflect_referrer_policy_attribute};
use crate::dom::html::links::htmlhyperlinkelementutils::{
    HyperlinkElement, HyperlinkElementTraits,
};
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svggraphicselement::SVGGraphicsElement;
use crate::dom::svg::svgtypes::{svg_animated_string, svg_uri_reference};

/// <https://svgwg.org/svg2-draft/linking.html#InterfaceSVGAElement>
#[dom_struct]
pub(crate) struct SVGAElement {
    svggraphicselement: SVGGraphicsElement,
    rel_list: MutNullableDom<DOMTokenList>,
    /// <https://html.spec.whatwg.org/multipage/#concept-hyperlink-url>
    #[no_trace]
    url: DomRefCell<Option<UrlWithBlobClaim>>,
}

impl HyperlinkElement for SVGAElement {
    fn get_url(&self) -> &DomRefCell<Option<UrlWithBlobClaim>> {
        &self.url
    }
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
            url: DomRefCell::new(None),
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

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-origin>
    fn Origin(&self, no_gc: &NoGC) -> USVString {
        self.get_origin(no_gc)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-protocol>
    fn Protocol(&self, no_gc: &NoGC) -> USVString {
        self.get_protocol(no_gc)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-protocol>
    fn SetProtocol(&self, cx: &mut js::context::JSContext, value: USVString) {
        self.set_protocol(cx, value);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-username>
    fn Username(&self, no_gc: &NoGC) -> USVString {
        self.get_username(no_gc)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-username>
    fn SetUsername(&self, cx: &mut js::context::JSContext, value: USVString) {
        self.set_username(cx, value);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-password>
    fn Password(&self, no_gc: &NoGC) -> USVString {
        self.get_password(no_gc)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-password>
    fn SetPassword(&self, cx: &mut js::context::JSContext, value: USVString) {
        self.set_password(cx, value);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-host>
    fn Host(&self, no_gc: &NoGC) -> USVString {
        self.get_host(no_gc)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-host>
    fn SetHost(&self, cx: &mut js::context::JSContext, value: USVString) {
        self.set_host(cx, value);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-hostname>
    fn Hostname(&self, no_gc: &NoGC) -> USVString {
        self.get_hostname(no_gc)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-hostname>
    fn SetHostname(&self, cx: &mut js::context::JSContext, value: USVString) {
        self.set_hostname(cx, value);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-port>
    fn Port(&self, no_gc: &NoGC) -> USVString {
        self.get_port(no_gc)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-port>
    fn SetPort(&self, cx: &mut js::context::JSContext, value: USVString) {
        self.set_port(cx, value);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-pathname>
    fn Pathname(&self, no_gc: &NoGC) -> USVString {
        self.get_pathname(no_gc)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-pathname>
    fn SetPathname(&self, cx: &mut js::context::JSContext, value: USVString) {
        self.set_pathname(cx, value);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-search>
    fn Search(&self, no_gc: &NoGC) -> USVString {
        self.get_search(no_gc)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-search>
    fn SetSearch(&self, cx: &mut js::context::JSContext, value: USVString) {
        self.set_search(cx, value);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-hash>
    fn Hash(&self, no_gc: &NoGC) -> USVString {
        self.get_hash(no_gc)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-hyperlink-hash>
    fn SetHash(&self, cx: &mut js::context::JSContext, value: USVString) {
        self.set_hash(cx, value);
    }
}
