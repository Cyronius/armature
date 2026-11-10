/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name, ns};
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::str::DOMString;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGScriptElementBinding::SVGScriptElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::element::attributes::storage::AttrRef;
use crate::dom::element::{
    AttributeMutation, Element, ElementCreator, reflect_cross_origin_attribute,
    reflect_fetch_priority_attribute, reflect_referrer_policy_attribute,
    set_cross_origin_attribute,
};
use crate::dom::html::htmlscriptelement::{ScriptElement, ScriptElementState};
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::node::{ChildrenMutation, CloneChildrenFlag, Node};
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::svg_uri_reference;

/// <https://svgwg.org/svg2-draft/interact.html#InterfaceSVGScriptElement>
#[dom_struct]
pub(crate) struct SVGScriptElement {
    svgelement: SVGElement,

    /// The state of the script processing model for this element.
    script_state: ScriptElementState,
}

impl SVGScriptElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        creator: ElementCreator,
    ) -> SVGScriptElement {
        SVGScriptElement {
            svgelement: SVGElement::new_inherited_with_state(
                ElementState::empty(),
                local_name,
                prefix,
                document,
            ),
            script_state: ScriptElementState::new(document, creator),
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
        creator: ElementCreator,
    ) -> DomRoot<SVGScriptElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGScriptElement::new_inherited(
                local_name, prefix, document, creator,
            )),
            document,
            proto,
        )
    }
}

impl ScriptElement for SVGScriptElement {
    fn element(&self) -> &Element {
        self.upcast()
    }

    fn script_state(&self) -> &ScriptElementState {
        &self.script_state
    }

    fn has_render_blocking_attribute(&self) -> bool {
        false
    }
}

impl VirtualMethods for SVGScriptElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }

    fn attribute_mutated(
        &self,
        cx: &mut JSContext,
        attr: AttrRef<'_>,
        mutation: AttributeMutation,
    ) {
        self.super_type()
            .unwrap()
            .attribute_mutated(cx, attr, mutation);
        if *attr.local_name() == local_name!("href") &&
            matches!(attr.namespace(), &ns!() | &ns!(xlink))
        {
            self.source_attribute_mutated(cx, mutation);
        }
    }

    fn children_changed(&self, cx: &mut JSContext, mutation: &ChildrenMutation) {
        if let Some(s) = self.super_type() {
            s.children_changed(cx, mutation);
        }

        self.script_children_changed();
    }

    fn post_connection_steps(&self, cx: &mut JSContext) {
        if let Some(s) = self.super_type() {
            s.post_connection_steps(cx);
        }

        self.script_post_connection_steps(cx);
    }

    fn cloning_steps(
        &self,
        cx: &mut JSContext,
        copy: &Node,
        maybe_doc: Option<&Document>,
        clone_children: CloneChildrenFlag,
    ) {
        if let Some(s) = self.super_type() {
            s.cloning_steps(cx, copy, maybe_doc, clone_children);
        }

        self.script_cloning_steps(copy);
    }
}

impl SVGScriptElementMethods<crate::DomTypeHolder> for SVGScriptElement {
    // <https://svgwg.org/svg2-draft/types.html#__svg__SVGURIReference__href>
    svg_uri_reference!();

    // https://svgwg.org/svg2-draft/interact.html#__svg__SVGScriptElement__type
    make_getter!(Type, "type");
    // https://svgwg.org/svg2-draft/interact.html#__svg__SVGScriptElement__type
    make_setter!(SetType, "type");

    /// <https://svgwg.org/svg2-draft/interact.html#__svg__SVGScriptElement__crossOrigin>
    fn GetCrossOrigin(&self) -> Option<DOMString> {
        reflect_cross_origin_attribute(self.upcast::<Element>())
    }

    /// <https://svgwg.org/svg2-draft/interact.html#__svg__SVGScriptElement__crossOrigin>
    fn SetCrossOrigin(&self, cx: &mut JSContext, value: Option<DOMString>) {
        set_cross_origin_attribute(cx, self.upcast::<Element>(), value);
    }

    /// <https://svgwg.org/svg2-draft/interact.html#__svg__SVGScriptElement__referrerPolicy>
    fn ReferrerPolicy(&self) -> DOMString {
        reflect_referrer_policy_attribute(self.upcast::<Element>())
    }

    // https://svgwg.org/svg2-draft/interact.html#__svg__SVGScriptElement__referrerPolicy
    make_setter!(SetReferrerPolicy, "referrerpolicy");

    /// <https://html.spec.whatwg.org/multipage/#dom-script-fetchpriority>
    fn FetchPriority(&self) -> DOMString {
        reflect_fetch_priority_attribute(self.upcast::<Element>())
    }

    // <https://html.spec.whatwg.org/multipage/#dom-script-fetchpriority>
    make_setter!(SetFetchPriority, "fetchpriority");
}
