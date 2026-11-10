/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name, ns};
use js::rust::HandleObject;

use crate::dom::activation::Activatable;
use crate::dom::bindings::codegen::Bindings::SVGUseElementBinding::SVGUseElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::element::Element;
use crate::dom::event::Event;
use crate::dom::eventtarget::EventTarget;
use crate::dom::iterators::ShadowIncluding;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgaelement::SVGAElement;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svggraphicselement::SVGGraphicsElement;
use crate::dom::svg::svgtypes::{SVGLengthValue, svg_animated_length, svg_uri_reference};

#[dom_struct]
pub(crate) struct SVGUseElement {
    svggraphicselement: SVGGraphicsElement,
}

impl SVGUseElement {
    fn referenced_anchor(&self) -> Option<DomRoot<SVGAElement>> {
        let element = self.upcast::<Element>();
        let href = element
            .get_attribute_string_value_with_namespace(&ns!(), &local_name!("href"))
            .or_else(|| {
                element.get_attribute_string_value_with_namespace(&ns!(xlink), &local_name!("href"))
            })?;
        let id = href.trim().strip_prefix('#')?;
        self.upcast::<Node>()
            .owner_doc()
            .upcast::<Node>()
            .traverse_preorder(ShadowIncluding::No)
            .filter_map(DomRoot::downcast::<Element>)
            .find(|candidate| candidate.get_id().is_some_and(|value| &*value == id))
            .and_then(DomRoot::downcast::<SVGAElement>)
    }

    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGUseElement {
        SVGUseElement {
            svggraphicselement: SVGGraphicsElement::new_inherited(local_name, prefix, document),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGUseElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGUseElement::new_inherited(local_name, prefix, document)),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGUseElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGraphicsElement>() as &dyn VirtualMethods)
    }
}

impl Activatable for SVGUseElement {
    fn as_element(&self) -> &Element {
        self.upcast::<Element>()
    }

    fn is_instance_activatable(&self) -> bool {
        self.referenced_anchor()
            .is_some_and(|anchor| anchor.is_instance_activatable())
    }

    fn activation_behavior(
        &self,
        cx: &mut js::context::JSContext,
        event: &Event,
        target: &EventTarget,
    ) {
        // The rendered copy has no DOM shadow tree, so use its source anchor.
        if let Some(anchor) = self.referenced_anchor() {
            anchor.activation_behavior(cx, event, target);
        }
    }
}

impl SVGUseElementMethods<crate::DomTypeHolder> for SVGUseElement {
    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGUseElement__instanceRoot>
    ///
    /// Servo does not create the shadow tree of `use` elements: it copies the referenced
    /// element when it serializes the SVG for rendering. So there is no instance root.
    fn GetInstanceRoot(&self) -> Option<crate::dom::bindings::root::DomRoot<SVGElement>> {
        None
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGUseElement__animatedInstanceRoot>
    fn GetAnimatedInstanceRoot(&self) -> Option<crate::dom::bindings::root::DomRoot<SVGElement>> {
        None
    }

    svg_animated_length!(X, "x", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Y, "y", Vertical, SVGLengthValue::number(0.0));
    svg_animated_length!(Width, "width", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Height, "height", Vertical, SVGLengthValue::number(0.0));

    // <https://svgwg.org/svg2-draft/types.html#__svg__SVGURIReference__href>
    svg_uri_reference!();
}
