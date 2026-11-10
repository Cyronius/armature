/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::DOMPointBinding::DOMPointInit;
use crate::dom::bindings::codegen::Bindings::SVGGeometryElementBinding::SVGGeometryElementMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::dompoint::DOMPoint;
use crate::dom::node::NodeTraits;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svggeometry;
use crate::dom::svg::svggraphicselement::SVGGraphicsElement;
use crate::dom::svg::svgtypes::svg_animated_number;

#[dom_struct]
pub(crate) struct SVGGeometryElement {
    svggraphicselement: SVGGraphicsElement,
}

impl SVGGeometryElement {
    pub(crate) fn new_inherited(
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGGeometryElement {
        SVGGeometryElement::new_inherited_with_state(
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
    ) -> SVGGeometryElement {
        SVGGeometryElement {
            svggraphicselement: SVGGraphicsElement::new_inherited_with_state(
                state, tag_name, prefix, document,
            ),
        }
    }
}

impl VirtualMethods for SVGGeometryElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGraphicsElement>() as &dyn VirtualMethods)
    }
}

impl SVGGeometryElement {
    fn geometry(&self, cx: &mut js::context::JSContext) -> Option<svggeometry::SVGShapeGeometry> {
        svggeometry::shape_geometry(cx, self.upcast())
    }
}

impl SVGGeometryElementMethods<crate::DomTypeHolder> for SVGGeometryElement {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGGeometryElement__isPointInFill>
    fn IsPointInFill(&self, cx: &mut js::context::JSContext, point: &DOMPointInit) -> bool {
        let Some(geometry) = self.geometry(cx) else {
            return false;
        };
        let point = kurbo::Point::new(point.x, point.y);
        if geometry.is_on_fill_boundary(point) {
            return true;
        }
        let winding = geometry.winding(point);
        let even_odd = svggeometry::computed_value(self.upcast(), "fill-rule") == "evenodd";
        if even_odd {
            winding % 2 != 0
        } else {
            winding != 0
        }
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGGeometryElement__isPointInStroke>
    fn IsPointInStroke(&self, cx: &mut js::context::JSContext, point: &DOMPointInit) -> bool {
        let Some(geometry) = self.geometry(cx) else {
            return false;
        };
        geometry.is_point_in_stroke(cx, self.upcast(), kurbo::Point::new(point.x, point.y))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGGeometryElement__getTotalLength>
    fn GetTotalLength(&self, cx: &mut js::context::JSContext) -> Finite<f32> {
        let length = self
            .geometry(cx)
            .map_or(0.0, |geometry| geometry.total_length());
        Finite::wrap(length as f32)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGGeometryElement__getPointAtLength>
    fn GetPointAtLength(
        &self,
        cx: &mut js::context::JSContext,
        distance: Finite<f32>,
    ) -> Fallible<DomRoot<DOMPoint>> {
        let point = self
            .geometry(cx)
            .and_then(|geometry| geometry.point_at_length(*distance as f64))
            .ok_or_else(|| {
                Error::InvalidState(Some("The shape has no point on its path".into()))
            })?;
        Ok(DOMPoint::new_svg(
            cx,
            &self.owner_global(),
            point.x,
            point.y,
        ))
    }

    svg_animated_number!(PathLength, "pathLength", "pathLength", Whole, 0.0);
}
