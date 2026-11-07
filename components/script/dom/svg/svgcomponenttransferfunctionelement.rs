/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGComponentTransferFunctionElementBinding::SVGComponentTransferFunctionElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::document::Document;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{
    svg_animated_enumeration, svg_animated_number, svg_animated_number_list,
};

/// <https://w3c.github.io/fxtf-drafts/filter-effects-1/#InterfaceSVGComponentTransferFunctionElement>
#[dom_struct]
pub(crate) struct SVGComponentTransferFunctionElement {
    svgelement: SVGElement,
}

impl SVGComponentTransferFunctionElement {
    pub(crate) fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGComponentTransferFunctionElement {
        SVGComponentTransferFunctionElement {
            svgelement: SVGElement::new_inherited_with_state(
                ElementState::empty(),
                local_name,
                prefix,
                document,
            ),
        }
    }
}

impl VirtualMethods for SVGComponentTransferFunctionElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGComponentTransferFunctionElementMethods<crate::DomTypeHolder>
    for SVGComponentTransferFunctionElement
{
    svg_animated_number_list!(TableValues, "tableValues", "tableValues");
    svg_animated_enumeration!(
        Type,
        "type",
        "type",
        &[
            ("identity", 1),
            ("table", 2),
            ("discrete", 3),
            ("linear", 4),
            ("gamma", 5)
        ],
        1
    );
    svg_animated_number!(Slope, "slope", "slope", Whole, 1.0);
    svg_animated_number!(Intercept, "intercept", "intercept", Whole, 0.0);
    svg_animated_number!(Amplitude, "amplitude", "amplitude", Whole, 1.0);
    svg_animated_number!(Exponent, "exponent", "exponent", Whole, 1.0);
    svg_animated_number!(Offset, "offset", "offset", Whole, 0.0);
}
