/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use euclid::default::Transform3D;
use html5ever::{LocalName, Prefix};
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::SVGGraphicsElementBinding::{
    SVGBoundingBoxOptions, SVGGraphicsElementMethods,
};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::dommatrix::DOMMatrix;
use crate::dom::domrect::DOMRect;
use crate::dom::node::NodeTraits;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svggeometry;
use crate::dom::svg::svgstringlist::SVGStringList;
use crate::dom::svg::svgtypes::svg_animated_transform_list;

#[dom_struct]
pub(crate) struct SVGGraphicsElement {
    svgelement: SVGElement,
}

impl SVGGraphicsElement {
    pub(crate) fn new_inherited(
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGGraphicsElement {
        SVGGraphicsElement::new_inherited_with_state(
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
    ) -> SVGGraphicsElement {
        SVGGraphicsElement {
            svgelement: SVGElement::new_inherited_with_state(state, tag_name, prefix, document),
        }
    }
}

impl VirtualMethods for SVGGraphicsElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }
}

/// Convert an affine transform into a 2D `DOMMatrix`.
fn affine_to_matrix(
    cx: &mut js::context::JSContext,
    element: &SVGGraphicsElement,
    transform: kurbo::Affine,
) -> DomRoot<DOMMatrix> {
    // Rotations by multiples of 90 degrees can leave tiny floating-point residues.
    // Keep the exact integer matrix components exposed by the SVG DOM APIs.
    let [a, b, c, d, e, f] = transform.as_coeffs().map(|value| {
        let rounded = value.round();
        if (value - rounded).abs() < 1e-12 {
            rounded
        } else {
            value
        }
    });
    DOMMatrix::new(
        cx,
        &element.owner_global(),
        true,
        Transform3D::new(
            a, b, 0.0, 0.0, c, d, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, e, f, 0.0, 1.0,
        ),
    )
}

impl SVGGraphicsElementMethods<crate::DomTypeHolder> for SVGGraphicsElement {
    svg_animated_transform_list!(Transform, "transform", "transform");

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGGraphicsElement__getBBox>
    fn GetBBox(
        &self,
        cx: &mut js::context::JSContext,
        options: &SVGBoundingBoxOptions,
    ) -> DomRoot<DOMRect> {
        // Make sure that the style of the element, which decides what is displayed, is up to
        // date.
        let element = self.upcast::<SVGElement>();
        let bbox = svggeometry::bounding_box_with_options(
            cx,
            element,
            options.fill,
            options.stroke,
            options.markers,
            options.clipped,
        )
        .unwrap_or_default();
        DOMRect::new_svg(
            cx,
            &self.owner_global(),
            bbox.x0,
            bbox.y0,
            bbox.width(),
            bbox.height(),
        )
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGGraphicsElement__getCTM>
    fn GetCTM(&self, cx: &mut js::context::JSContext) -> Option<DomRoot<DOMMatrix>> {
        let transform = svggeometry::current_transform(cx, self.upcast(), false);
        Some(affine_to_matrix(cx, self, transform))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGGraphicsElement__getScreenCTM>
    fn GetScreenCTM(&self, cx: &mut js::context::JSContext) -> Option<DomRoot<DOMMatrix>> {
        let transform = svggeometry::current_transform(cx, self.upcast(), true);
        Some(affine_to_matrix(cx, self, transform))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGTests__requiredExtensions>
    fn RequiredExtensions(&self, cx: &mut js::context::JSContext) -> DomRoot<SVGStringList> {
        self.upcast::<SVGElement>()
            .string_list(cx, "requiredExtensions", false)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGTests__systemLanguage>
    fn SystemLanguage(&self, cx: &mut js::context::JSContext) -> DomRoot<SVGStringList> {
        self.upcast::<SVGElement>()
            .string_list(cx, "systemLanguage", true)
    }
}
