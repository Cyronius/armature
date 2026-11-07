/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name};
use js::rust::HandleObject;
use script_bindings::str::DOMString;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGMarkerElementBinding::SVGMarkerElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgangle::SVGAngle;
use crate::dom::svg::svganimatedangle::SVGAnimatedAngle;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{
    SVGLengthValue, svg_animated_enumeration, svg_animated_length, svg_fit_to_view_box,
};

/// <https://svgwg.org/svg2-draft/painting.html#InterfaceSVGMarkerElement>
#[dom_struct]
pub(crate) struct SVGMarkerElement {
    svgelement: SVGElement,
}

impl SVGMarkerElement {
    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGMarkerElement {
        SVGMarkerElement {
            svgelement: SVGElement::new_inherited_with_state(
                ElementState::empty(),
                local_name,
                prefix,
                document,
            ),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGMarkerElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGMarkerElement::new_inherited(
                local_name, prefix, document,
            )),
            document,
            proto,
        )
    }
}

impl VirtualMethods for SVGMarkerElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

impl SVGMarkerElementMethods<crate::DomTypeHolder> for SVGMarkerElement {
    /// <https://svgwg.org/svg2-draft/painting.html#__svg__SVGMarkerElement__orientAngle>
    fn OrientAngle(&self, cx: &mut js::context::JSContext) -> DomRoot<SVGAnimatedAngle> {
        self.upcast::<SVGElement>()
            .animated_angle(cx, "orientAngle", local_name!("orient"))
    }

    /// <https://svgwg.org/svg2-draft/painting.html#__svg__SVGMarkerElement__orient>
    fn Orient(&self) -> DOMString {
        self.upcast::<SVGElement>()
            .svg_attribute(&local_name!("orient"))
            .map(DOMString::from)
            .unwrap_or_default()
    }

    /// <https://svgwg.org/svg2-draft/painting.html#__svg__SVGMarkerElement__orient>
    fn SetOrient(&self, cx: &mut js::context::JSContext, value: DOMString) {
        self.upcast::<SVGElement>().set_svg_attribute(
            cx,
            &local_name!("orient"),
            String::from(value.str().as_ref()),
        );
    }

    /// <https://svgwg.org/svg2-draft/painting.html#__svg__SVGMarkerElement__setOrientToAuto>
    fn SetOrientToAuto(&self, cx: &mut js::context::JSContext) {
        self.upcast::<SVGElement>().set_svg_attribute(
            cx,
            &local_name!("orient"),
            "auto".to_owned(),
        );
    }

    /// <https://svgwg.org/svg2-draft/painting.html#__svg__SVGMarkerElement__setOrientToAngle>
    fn SetOrientToAngle(&self, cx: &mut js::context::JSContext, angle: &SVGAngle) {
        self.upcast::<SVGElement>().set_svg_attribute(
            cx,
            &local_name!("orient"),
            angle.angle().to_string(),
        );
    }

    svg_fit_to_view_box!();

    svg_animated_length!(RefX, "refX", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(RefY, "refY", Vertical, SVGLengthValue::number(0.0));
    svg_animated_enumeration!(
        MarkerUnits,
        "markerUnits",
        "markerUnits",
        &[("userSpaceOnUse", 1), ("strokeWidth", 2)],
        2
    );
    svg_animated_length!(
        MarkerWidth,
        "markerWidth",
        Horizontal,
        SVGLengthValue::number(3.0)
    );
    svg_animated_length!(
        MarkerHeight,
        "markerHeight",
        Vertical,
        SVGLengthValue::number(3.0)
    );
    svg_animated_enumeration!(
        OrientType,
        "orientType",
        "orient",
        &[("auto", 1), ("auto-start-reverse", 3)],
        2
    );
}
