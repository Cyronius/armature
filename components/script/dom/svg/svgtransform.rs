/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use euclid::default::Transform3D;
use js::context::JSContext;
use kurbo::Affine;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::DOMMatrixBinding::DOMMatrix2DInit;
use crate::dom::bindings::codegen::Bindings::SVGTransformBinding::SVGTransformMethods;
use crate::dom::bindings::error::{Error, ErrorResult};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::dommatrix::DOMMatrix;
use crate::dom::dommatrixreadonly::{DOMMatrixReadOnly, dommatrix2dinit_to_matrix};
use crate::dom::globalscope::GlobalScope;
use crate::dom::svg::svgtransformlist::SVGTransformList;
use crate::dom::svg::svgtypes::number_to_string;

pub(crate) const SVG_TRANSFORM_MATRIX: u16 = 1;
pub(crate) const SVG_TRANSFORM_TRANSLATE: u16 = 2;
pub(crate) const SVG_TRANSFORM_SCALE: u16 = 3;
pub(crate) const SVG_TRANSFORM_ROTATE: u16 = 4;
pub(crate) const SVG_TRANSFORM_SKEWX: u16 = 5;
pub(crate) const SVG_TRANSFORM_SKEWY: u16 = 6;

/// The value of an `SVGTransform`: an item of a transform list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SVGTransformValue {
    pub(crate) kind: u16,
    /// For rotations and skews, the angle in degrees.
    pub(crate) angle: f64,
    /// For rotations, the center of rotation.
    pub(crate) center: (f64, f64),
    pub(crate) matrix: Affine,
}

impl SVGTransformValue {
    pub(crate) fn matrix(matrix: Affine) -> Self {
        Self {
            kind: SVG_TRANSFORM_MATRIX,
            angle: 0.0,
            center: (0.0, 0.0),
            matrix,
        }
    }

    pub(crate) fn translate(x: f64, y: f64) -> Self {
        Self {
            kind: SVG_TRANSFORM_TRANSLATE,
            matrix: Affine::translate((x, y)),
            ..Self::matrix(Affine::IDENTITY)
        }
    }

    pub(crate) fn scale(x: f64, y: f64) -> Self {
        Self {
            kind: SVG_TRANSFORM_SCALE,
            matrix: Affine::scale_non_uniform(x, y),
            ..Self::matrix(Affine::IDENTITY)
        }
    }

    pub(crate) fn rotate(angle: f64, x: f64, y: f64) -> Self {
        Self {
            kind: SVG_TRANSFORM_ROTATE,
            angle,
            center: (x, y),
            matrix: Affine::translate((x, y)) *
                Affine::rotate(angle.to_radians()) *
                Affine::translate((-x, -y)),
        }
    }

    pub(crate) fn skew_x(angle: f64) -> Self {
        Self {
            kind: SVG_TRANSFORM_SKEWX,
            angle,
            center: (0.0, 0.0),
            matrix: Affine::skew(angle.to_radians().tan(), 0.0),
        }
    }

    pub(crate) fn skew_y(angle: f64) -> Self {
        Self {
            kind: SVG_TRANSFORM_SKEWY,
            angle,
            center: (0.0, 0.0),
            matrix: Affine::skew(0.0, angle.to_radians().tan()),
        }
    }
}

impl std::fmt::Display for SVGTransformValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = |value: f64| number_to_string(value as f32);
        let [a, b, c, d, e, g] = self.matrix.as_coeffs();
        match self.kind {
            SVG_TRANSFORM_TRANSLATE => write!(f, "translate({} {})", n(e), n(g)),
            SVG_TRANSFORM_SCALE => write!(f, "scale({} {})", n(a), n(d)),
            SVG_TRANSFORM_ROTATE if self.center == (0.0, 0.0) => {
                write!(f, "rotate({})", n(self.angle))
            },
            SVG_TRANSFORM_ROTATE => write!(
                f,
                "rotate({} {} {})",
                n(self.angle),
                n(self.center.0),
                n(self.center.1)
            ),
            SVG_TRANSFORM_SKEWX => write!(f, "skewX({})", n(self.angle)),
            SVG_TRANSFORM_SKEWY => write!(f, "skewY({})", n(self.angle)),
            _ => write!(
                f,
                "matrix({} {} {} {} {} {})",
                n(a),
                n(b),
                n(c),
                n(d),
                n(e),
                n(g)
            ),
        }
    }
}

/// Convert an affine transform into the matrix of a 2D `DOMMatrix`.
pub(crate) fn affine_to_transform3d(affine: Affine) -> Transform3D<f64> {
    let [a, b, c, d, e, f] = affine.as_coeffs();
    Transform3D::new(
        a, b, 0.0, 0.0, c, d, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, e, f, 0.0, 1.0,
    )
}

/// <https://svgwg.org/svg2-draft/coords.html#InterfaceSVGTransform>
#[dom_struct]
pub(crate) struct SVGTransform {
    reflector_: Reflector,
    kind: Cell<u16>,
    angle: Cell<f64>,
    center: Cell<(f64, f64)>,
    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransform__matrix>
    ///
    /// The matrix also stores the transform: changing it makes this a matrix transform.
    matrix: Dom<DOMMatrix>,
    /// The list that this transform is an element of, which reflects an attribute.
    list: MutNullableDom<SVGTransformList>,
    read_only: Cell<bool>,
}

impl SVGTransform {
    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        value: SVGTransformValue,
        list: Option<&SVGTransformList>,
        read_only: bool,
    ) -> DomRoot<Self> {
        let matrix = DOMMatrix::new(cx, global, true, affine_to_transform3d(value.matrix));
        let transform = reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                kind: Cell::new(value.kind),
                angle: Cell::new(value.angle),
                center: Cell::new(value.center),
                matrix: Dom::from_ref(&*matrix),
                list: MutNullableDom::new(list),
                read_only: Cell::new(read_only),
            }),
            global,
        );
        matrix.set_svg_transform(&transform);
        transform
    }

    /// The value, without synchronizing a list that this transform is an element of first.
    pub(crate) fn value(&self) -> SVGTransformValue {
        let matrix = self.matrix.upcast::<DOMMatrixReadOnly>().matrix();
        SVGTransformValue {
            kind: self.kind.get(),
            angle: self.angle.get(),
            center: self.center.get(),
            matrix: Affine::new([
                matrix.m11, matrix.m12, matrix.m21, matrix.m22, matrix.m41, matrix.m42,
            ]),
        }
    }

    /// Set the value, without serializing a list that this transform is an element of.
    pub(crate) fn set_raw_value(&self, value: SVGTransformValue) {
        self.kind.set(value.kind);
        self.angle.set(value.angle);
        self.center.set(value.center);
        self.matrix
            .upcast::<DOMMatrixReadOnly>()
            .set_matrix(affine_to_transform3d(value.matrix));
    }

    pub(crate) fn is_attached(&self) -> bool {
        self.list.get().is_some() || self.read_only.get()
    }

    pub(crate) fn attach(&self, list: &SVGTransformList, read_only: bool) {
        self.list.set(Some(list));
        self.read_only.set(read_only);
    }

    /// <https://svgwg.org/svg2-draft/types.html#TermDetach>
    pub(crate) fn detach(&self) {
        self.list.set(None);
        self.read_only.set(false);
    }

    fn synchronize(&self, cx: &mut JSContext) {
        if let Some(list) = self.list.get() {
            list.synchronize(cx);
        }
    }

    /// Set the value, and serialize the list that this transform is an element of.
    fn set_value(&self, cx: &mut JSContext, value: SVGTransformValue) -> ErrorResult {
        if self.read_only.get() {
            return Err(Error::NoModificationAllowed(Some(
                "This SVGTransform is read-only".into(),
            )));
        }
        self.synchronize(cx);
        self.set_raw_value(value);
        if let Some(list) = self.list.get() {
            list.commit(cx);
        }
        Ok(())
    }

    /// Called when script changed the matrix: this becomes a matrix transform.
    ///
    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransform__matrix>
    pub(crate) fn matrix_changed(&self, cx: &mut JSContext) {
        self.kind.set(SVG_TRANSFORM_MATRIX);
        self.angle.set(0.0);
        self.center.set((0.0, 0.0));
        if let Some(list) = self.list.get() {
            list.commit(cx);
        }
    }

    /// Whether script may change the matrix.
    pub(crate) fn is_read_only(&self) -> bool {
        self.read_only.get()
    }
}

impl SVGTransformMethods<crate::DomTypeHolder> for SVGTransform {
    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransform__type>
    fn Type(&self, cx: &mut JSContext) -> u16 {
        self.synchronize(cx);
        self.kind.get()
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransform__matrix>
    fn Matrix(&self, cx: &mut JSContext) -> DomRoot<DOMMatrix> {
        self.synchronize(cx);
        self.matrix.as_rooted()
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransform__angle>
    fn Angle(&self, cx: &mut JSContext) -> Finite<f32> {
        self.synchronize(cx);
        Finite::wrap(self.angle.get() as f32)
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransform__setMatrix>
    fn SetMatrix(&self, cx: &mut JSContext, matrix: &DOMMatrix2DInit) -> ErrorResult {
        let matrix = dommatrix2dinit_to_matrix(matrix)?;
        self.set_value(
            cx,
            SVGTransformValue::matrix(Affine::new([
                matrix.m11, matrix.m12, matrix.m21, matrix.m22, matrix.m31, matrix.m32,
            ])),
        )
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransform__setTranslate>
    fn SetTranslate(&self, cx: &mut JSContext, tx: Finite<f32>, ty: Finite<f32>) -> ErrorResult {
        self.set_value(cx, SVGTransformValue::translate(*tx as f64, *ty as f64))
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransform__setScale>
    fn SetScale(&self, cx: &mut JSContext, sx: Finite<f32>, sy: Finite<f32>) -> ErrorResult {
        self.set_value(cx, SVGTransformValue::scale(*sx as f64, *sy as f64))
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransform__setRotate>
    fn SetRotate(
        &self,
        cx: &mut JSContext,
        angle: Finite<f32>,
        x: Finite<f32>,
        y: Finite<f32>,
    ) -> ErrorResult {
        self.set_value(
            cx,
            SVGTransformValue::rotate(*angle as f64, *x as f64, *y as f64),
        )
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransform__setSkewX>
    fn SetSkewX(&self, cx: &mut JSContext, angle: Finite<f32>) -> ErrorResult {
        self.set_value(cx, SVGTransformValue::skew_x(*angle as f64))
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGTransform__setSkewY>
    fn SetSkewY(&self, cx: &mut JSContext, angle: Finite<f32>) -> ErrorResult {
        self.set_value(cx, SVGTransformValue::skew_y(*angle as f64))
    }
}
