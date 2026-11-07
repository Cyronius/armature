/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGTextContentElementBinding::SVGTextContentElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::document::Document;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svggraphicselement::SVGGraphicsElement;
use crate::dom::svg::svgtypes::{SVGLengthValue, svg_animated_enumeration, svg_animated_length};

#[dom_struct]
pub(crate) struct SVGTextContentElement {
    svggraphicselement: SVGGraphicsElement,
}

impl SVGTextContentElement {
    pub(crate) fn new_inherited(
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGTextContentElement {
        SVGTextContentElement::new_inherited_with_state(
            ElementState::empty(),
            tag_name,
            prefix,
            document,
        )
    }

    pub(crate) fn new_inherited_with_state(
        state: ElementState,
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGTextContentElement {
        SVGTextContentElement {
            svggraphicselement: SVGGraphicsElement::new_inherited_with_state(
                state, tag_name, prefix, document,
            ),
        }
    }
}

impl VirtualMethods for SVGTextContentElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGraphicsElement>() as &dyn VirtualMethods)
    }
}

impl SVGTextContentElementMethods<crate::DomTypeHolder> for SVGTextContentElement {
    svg_animated_length!(
        TextLength,
        "textLength",
        Horizontal,
        SVGLengthValue::number(0.0)
    );
    svg_animated_enumeration!(
        LengthAdjust,
        "lengthAdjust",
        "lengthAdjust",
        &[("spacing", 1), ("spacingAndGlyphs", 2)],
        1
    );
}
