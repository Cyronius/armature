/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name};
use js::context::JSContext;
use js::rust::HandleObject;
use style::attr::{AttrValue, LengthOrPercentageOrAuto};

use crate::dom::bindings::codegen::Bindings::HTMLMarqueeElementBinding::HTMLMarqueeElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{DomRoot, LayoutDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::element::Element;
use crate::dom::element::attributes::storage::AttrRef;
use crate::dom::html::htmlelement::HTMLElement;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::values::UNSIGNED_LONG_MAX;

const DEFAULT_SCROLLAMOUNT: u32 = 6;
const DEFAULT_SCROLLDELAY: u32 = 85;

#[dom_struct]
pub(crate) struct HTMLMarqueeElement {
    htmlelement: HTMLElement,
}

impl HTMLMarqueeElement {
    fn new_inherited(local_name: LocalName, prefix: Option<Prefix>, document: &Document) -> Self {
        Self {
            htmlelement: HTMLElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<Self> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(Self::new_inherited(local_name, prefix, document)),
            document,
            proto,
        )
    }
}

impl HTMLMarqueeElementMethods<crate::DomTypeHolder> for HTMLMarqueeElement {
    // <https://html.spec.whatwg.org/multipage/#dom-marquee-behavior>
    fn Behavior(&self) -> DOMString {
        let value = self
            .upcast::<Element>()
            .get_string_attribute(&LocalName::from("behavior"));
        match &*value.str().to_ascii_lowercase() {
            keyword @ ("scroll" | "slide" | "alternate") => DOMString::from(keyword),
            _ => DOMString::from_static("scroll"),
        }
    }

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-behavior>
    fn SetBehavior(&self, cx: &mut JSContext, value: DOMString) {
        self.upcast::<Element>()
            .set_string_attribute(cx, &LocalName::from("behavior"), value)
    }

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-bgcolor>
    make_getter!(BgColor, "bgcolor");

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-bgcolor>
    make_setter!(SetBgColor, "bgcolor");

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-direction>
    make_enumerated_getter!(
        Direction,
        "direction",
        "up" | "right" | "down" | "left",
        missing => "left",
        invalid => "left"
    );

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-direction>
    make_setter!(SetDirection, "direction");

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-scrollamount>
    fn ScrollAmount(&self) -> u32 {
        self.upcast::<Element>()
            .get_uint_attribute(&LocalName::from("scrollamount"), DEFAULT_SCROLLAMOUNT)
    }

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-scrollamount>
    fn SetScrollAmount(&self, cx: &mut JSContext, value: u32) {
        let value = if value > UNSIGNED_LONG_MAX {
            DEFAULT_SCROLLAMOUNT
        } else {
            value
        };
        self.upcast::<Element>()
            .set_attribute(cx, &LocalName::from("scrollamount"), value.into())
    }

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-scrolldelay>
    make_uint_getter!(ScrollDelay, "scrolldelay", DEFAULT_SCROLLDELAY);

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-scrolldelay>
    make_uint_setter!(SetScrollDelay, "scrolldelay", DEFAULT_SCROLLDELAY);

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-truespeed>
    fn TrueSpeed(&self) -> bool {
        self.upcast::<Element>()
            .has_attribute(&LocalName::from("truespeed"))
    }

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-truespeed>
    fn SetTrueSpeed(&self, cx: &mut JSContext, value: bool) {
        self.upcast::<Element>()
            .set_bool_attribute(cx, &LocalName::from("truespeed"), value)
    }

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-width>
    make_getter!(Width, "width");
    // <https://html.spec.whatwg.org/multipage/#dom-marquee-width>
    make_dimension_setter!(SetWidth, "width");

    // <https://html.spec.whatwg.org/multipage/#dom-marquee-height>
    make_getter!(Height, "height");
    // <https://html.spec.whatwg.org/multipage/#dom-marquee-height>
    make_dimension_setter!(SetHeight, "height");

    // https://html.spec.whatwg.org/multipage/#dom-marquee-hspace
    make_dimension_uint_getter!(Hspace, "hspace");

    // https://html.spec.whatwg.org/multipage/#dom-marquee-hspace
    make_dimension_uint_setter!(SetHspace, "hspace");

    // https://html.spec.whatwg.org/multipage/#dom-marquee-vspace
    make_dimension_uint_getter!(Vspace, "vspace");

    // https://html.spec.whatwg.org/multipage/#dom-marquee-vspace
    make_dimension_uint_setter!(SetVspace, "vspace");
}

impl VirtualMethods for HTMLMarqueeElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<HTMLElement>() as &dyn VirtualMethods)
    }

    fn attribute_affects_presentational_hints(&self, attr: AttrRef<'_>) -> bool {
        match attr.local_name() {
            &local_name!("width") |
            &local_name!("height") |
            &local_name!("hspace") |
            &local_name!("vspace") => true,
            _ => self
                .super_type()
                .unwrap()
                .attribute_affects_presentational_hints(attr),
        }
    }

    fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue {
        match *name {
            local_name!("width") |
            local_name!("height") |
            local_name!("hspace") |
            local_name!("vspace") => AttrValue::from_dimension(value.into()),
            local_name!("scrolldelay") => AttrValue::from_u32(value.into(), DEFAULT_SCROLLDELAY),
            _ if &**name == "scrollamount" => {
                AttrValue::from_u32(value.into(), DEFAULT_SCROLLAMOUNT)
            },
            _ => self
                .super_type()
                .unwrap()
                .parse_plain_attribute(name, value),
        }
    }
}

impl LayoutDom<'_, HTMLMarqueeElement> {
    pub(crate) fn width(self) -> LengthOrPercentageOrAuto {
        self.upcast::<Element>()
            .dimension_attr_value(local_name!("width"))
    }

    pub(crate) fn height(self) -> LengthOrPercentageOrAuto {
        self.upcast::<Element>()
            .dimension_attr_value(local_name!("height"))
    }

    pub(crate) fn margin_right_left(self) -> LengthOrPercentageOrAuto {
        self.upcast::<Element>()
            .dimension_attr_value(local_name!("hspace"))
    }

    pub(crate) fn margin_top_bottom(self) -> LengthOrPercentageOrAuto {
        self.upcast::<Element>()
            .dimension_attr_value(local_name!("vspace"))
    }
}
