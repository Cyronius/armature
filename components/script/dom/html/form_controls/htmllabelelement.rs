/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name, ns};
use js::context::NoGC;
use js::rust::HandleObject;
use style::attr::AttrValue;

use crate::dom::activation::Activatable;
use crate::dom::bindings::codegen::Bindings::ElementBinding::ElementMethods;
use crate::dom::bindings::codegen::Bindings::EventBinding::EventMethods;
use crate::dom::bindings::codegen::Bindings::HTMLElementBinding::HTMLElementMethods;
use crate::dom::bindings::codegen::Bindings::HTMLLabelElementBinding::HTMLLabelElementMethods;
use crate::dom::bindings::codegen::Bindings::NodeBinding::{GetRootNodeOptions, NodeMethods};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::element::attributes::storage::AttrRef;
use crate::dom::element::{AttributeMutation, Element};
use crate::dom::event::Event;
use crate::dom::eventtarget::EventTarget;
use crate::dom::html::htmlelement::HTMLElement;
use crate::dom::html::htmlformelement::{FormControlElementHelpers, HTMLFormElement};
use crate::dom::iterators::ShadowIncluding;
use crate::dom::mouseevent::MouseEvent;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;

#[dom_struct]
pub(crate) struct HTMLLabelElement {
    htmlelement: HTMLElement,
}

impl HTMLLabelElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> HTMLLabelElement {
        HTMLLabelElement {
            htmlelement: HTMLElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<HTMLLabelElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(HTMLLabelElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl Activatable for HTMLLabelElement {
    fn as_element(&self) -> &Element {
        self.upcast::<Element>()
    }

    fn is_instance_activatable(&self) -> bool {
        // An empty label inside another label still absorbs activation: it is
        // interactive content and must not activate the outer label's control.
        // Otherwise, let an enclosing button handle a click on an empty label.
        self.upcast::<Element>().has_attribute(&local_name!("for")) ||
            self.first_labelable_descendant().is_some() ||
            self.upcast::<Node>()
                .ancestors()
                .any(|ancestor| ancestor.is::<HTMLLabelElement>())
    }

    // https://html.spec.whatwg.org/multipage/#the-label-element:activation_behaviour
    // Basically this is telling us that if activation bubbles up to the label
    // at all, we are free to do an implementation-dependent thing;
    // firing a click event is an example, and the precise details of that
    // click event (e.g. isTrusted) are not specified.
    fn activation_behavior(
        &self,
        cx: &mut js::context::JSContext,
        event: &Event,
        target: &EventTarget,
    ) {
        let Some(control) = self.GetControl(cx.no_gc()) else {
            return;
        };
        // Clicks on the labeled control, or on interactive content inside the label, are not
        // forwarded (Gecko's HTMLLabelElement::PostHandleEvent and InInteractiveHTMLContent).
        if let Some(target) = target.downcast::<Node>() {
            let label = self.upcast::<Node>();
            let on_interactive_content = target
                .inclusive_ancestors(ShadowIncluding::No)
                .take_while(|node| &**node != label)
                .any(|node| {
                    &*node == control.upcast::<Node>() ||
                        node.downcast::<Element>().is_some_and(is_interactive_content)
                });
            if on_interactive_content {
                return;
            }
        }
        // A click from the user is forwarded as a trusted click that keeps its pointer
        // properties, as Gecko's HTMLLabelElement::PostHandleEvent does.
        if event.IsTrusted() &&
            let Some(mouse_event) = event.downcast::<MouseEvent>()
        {
            if control.upcast::<Element>().disabled_state() {
                return;
            }
            mouse_event
                .forwarded_click(cx)
                .dispatch(cx, control.upcast::<EventTarget>(), false);
            return;
        }
        control.Click(cx);
    }
}

impl HTMLLabelElementMethods<crate::DomTypeHolder> for HTMLLabelElement {
    /// <https://html.spec.whatwg.org/multipage/#dom-label-form>
    fn GetForm(&self, no_gc: &NoGC) -> Option<DomRoot<HTMLFormElement>> {
        // > The form IDL attribute must run the following steps:
        // > 1. If the label element has no labeled control, then return null.
        // > 2. If the label element's labeled control is not a form-associated element, then
        // >    return null.
        // > 3. Return the label element's labeled control's form owner (which can still be
        // >    null).
        self.GetControl(no_gc)
            .map(DomRoot::upcast::<Element>)
            .and_then(|element| {
                element
                    .as_maybe_form_control()
                    .and_then(|control| control.form_owner())
            })
    }

    // https://html.spec.whatwg.org/multipage/#dom-label-htmlfor
    make_getter!(HtmlFor, "for");

    // https://html.spec.whatwg.org/multipage/#dom-label-htmlfor
    make_atomic_setter!(SetHtmlFor, "for");

    /// <https://html.spec.whatwg.org/multipage/#dom-label-control>
    fn GetControl(&self, no_gc: &NoGC) -> Option<DomRoot<HTMLElement>> {
        let Some(for_value) = self
            .upcast::<Element>()
            .get_attribute_string_value(&local_name!("for"))
        else {
            return self.first_labelable_descendant();
        };

        // "If the attribute is specified and there is an element in the tree
        // whose ID is equal to the value of the for attribute, and the first
        // such element in tree order is a labelable element, then that
        // element is the label element's labeled control."
        // Two subtle points here: we need to search the _tree_, which is
        // not necessarily the document if we're detached from the document,
        // and we only consider one element even if a later element with
        // the same ID is labelable.

        let maybe_found = self
            .upcast::<Node>()
            .GetRootNode(&GetRootNodeOptions::empty())
            .traverse_preorder_unrooted(no_gc, ShadowIncluding::No)
            .find_map(|e| {
                if let Some(htmle) = e.downcast::<HTMLElement>() {
                    if htmle.upcast::<Element>().Id() == for_value {
                        Some(DomRoot::from_ref(htmle))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
        // We now have the element that we would return, but only return it
        // if it's labelable.
        if let Some(ref maybe_labelable) = maybe_found &&
            maybe_labelable.is_labelable_element()
        {
            return maybe_found;
        }
        None
    }
}

impl VirtualMethods for HTMLLabelElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<HTMLElement>() as &dyn VirtualMethods)
    }

    fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue {
        match name {
            &local_name!("for") => AttrValue::from_atomic(value.into()),
            _ => self
                .super_type()
                .unwrap()
                .parse_plain_attribute(name, value),
        }
    }

    fn attribute_mutated(
        &self,
        cx: &mut js::context::JSContext,
        attr: AttrRef<'_>,
        mutation: AttributeMutation,
    ) {
        self.super_type()
            .unwrap()
            .attribute_mutated(cx, attr, mutation);
    }
}

impl HTMLLabelElement {
    pub(crate) fn first_labelable_descendant(&self) -> Option<DomRoot<HTMLElement>> {
        self.upcast::<Node>()
            .traverse_preorder(ShadowIncluding::No)
            .filter_map(DomRoot::downcast::<HTMLElement>)
            .find(|elem| elem.is_labelable_element())
    }
}

/// <https://html.spec.whatwg.org/multipage/#interactive-content-2>
pub(crate) fn is_interactive_content(element: &Element) -> bool {
    if *element.namespace() != ns!(html) {
        return false;
    }
    let has = |name: &LocalName| element.has_attribute(name);
    match element.local_name() {
        &local_name!("a") => has(&local_name!("href")),
        &local_name!("audio") | &local_name!("video") => has(&local_name!("controls")),
        &local_name!("img") => has(&local_name!("usemap")),
        &local_name!("input") => !element
            .get_attribute_string_value(&local_name!("type"))
            .is_some_and(|value| value.eq_ignore_ascii_case("hidden")),
        &local_name!("button") |
        &local_name!("details") |
        &local_name!("embed") |
        &local_name!("iframe") |
        &local_name!("label") |
        &local_name!("select") |
        &local_name!("textarea") => true,
        _ => false,
    }
}
