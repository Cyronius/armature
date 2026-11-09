/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::{JSContext, NoGC};
use js::rust::HandleObject;
use rustc_hash::FxHashMap;
use script_bindings::reflector::{reflect_dom_object, reflect_dom_object_with_proto};
use script_bindings::serializable::Serializable;
use script_bindings::structuredclone::StructuredData;
use servo_base::id::{DomRectId, DomRectIndex};
use servo_constellation_traits::DomRect;

use crate::dom::bindings::codegen::Bindings::DOMRectBinding::DOMRectMethods;
use crate::dom::bindings::codegen::Bindings::DOMRectReadOnlyBinding::{
    DOMRectInit, DOMRectReadOnlyMethods,
};
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::domrectreadonly::{DOMRectReadOnly, create_a_domrectreadonly_from_the_dictionary};
use crate::dom::globalscope::GlobalScope;
use crate::dom::svg::svganimatedrect::SVGAnimatedRect;

#[dom_struct]
pub(crate) struct DOMRect {
    rect: DOMRectReadOnly,
    /// For the base or animated value of an `SVGAnimatedRect`, which the SVG DOM calls an
    /// `SVGRect`, the object whose attribute the coordinates reflect.
    svg_animated_rect: MutNullableDom<SVGAnimatedRect>,
    /// SVG rectangles reject non-finite coordinates, unlike ordinary DOM rectangles.
    svg_rect: bool,
    /// Whether this is the animated value of an `SVGAnimatedRect`, which cannot be changed.
    svg_read_only: bool,
}

impl DOMRect {
    fn new_inherited(x: f64, y: f64, width: f64, height: f64, svg_rect: bool) -> DOMRect {
        DOMRect {
            rect: DOMRectReadOnly::new_inherited(x, y, width, height),
            svg_animated_rect: Default::default(),
            svg_rect,
            svg_read_only: false,
        }
    }

    /// Create the base value, or with `read_only` the animated value, of `animated`.
    #[cfg_attr(crown, expect(crown::unrooted_must_root))]
    pub(crate) fn new_reflecting_svg_rect(
        cx: &mut JSContext,
        animated: &SVGAnimatedRect,
        read_only: bool,
    ) -> DomRoot<DOMRect> {
        let rect = DOMRect {
            rect: DOMRectReadOnly::new_inherited(0.0, 0.0, 0.0, 0.0),
            svg_animated_rect: MutNullableDom::new(Some(animated)),
            svg_rect: true,
            svg_read_only: read_only,
        };
        reflect_dom_object(cx, Box::new(rect), &*animated.global())
    }

    /// The coordinate with index `index` (x, y, width, height).
    fn coordinate(&self, cx: &mut JSContext, index: usize) -> f64 {
        if let Some(animated) = self.svg_animated_rect.get() {
            // Keep the coordinates of DOMRectReadOnly, which `top`, `left`, `right` and
            // `bottom` use, in sync with the attribute as well.
            let rect = if self.svg_read_only {
                animated.animated_rect(cx)
            } else {
                animated.rect()
            };
            self.rect.set_x(rect[0]);
            self.rect.set_y(rect[1]);
            self.rect.set_width(rect[2]);
            self.rect.set_height(rect[3]);
            return rect[index];
        }
        match index {
            0 => self.rect.X(),
            1 => self.rect.Y(),
            2 => self.rect.Width(),
            _ => self.rect.Height(),
        }
    }

    /// Set the coordinate with index `index` (x, y, width, height).
    fn set_coordinate(&self, cx: &mut JSContext, index: usize, value: f64) -> ErrorResult {
        if self.svg_read_only {
            return Err(Error::NoModificationAllowed(Some(
                "This SVGRect is read-only".into(),
            )));
        }
        if self.svg_rect && !value.is_finite() {
            return Err(Error::Type(
                c"SVGRect coordinates must be finite".to_owned(),
            ));
        }
        if let Some(animated) = self.svg_animated_rect.get() {
            animated.set_coordinate(cx, index, value);
            return Ok(());
        }
        match index {
            0 => self.rect.set_x(value),
            1 => self.rect.set_y(value),
            2 => self.rect.set_width(value),
            _ => self.rect.set_height(value),
        }
        Ok(())
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> DomRoot<DOMRect> {
        Self::new_with_proto(cx, global, None, x, y, width, height)
    }

    pub(crate) fn new_svg(
        cx: &mut JSContext,
        global: &GlobalScope,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> DomRoot<DOMRect> {
        reflect_dom_object(
            cx,
            Box::new(Self::new_inherited(x, y, width, height, true)),
            global,
        )
    }

    fn new_with_proto(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> DomRoot<DOMRect> {
        reflect_dom_object_with_proto(
            cx,
            Box::new(DOMRect::new_inherited(x, y, width, height, false)),
            global,
            proto,
        )
    }
}

impl DOMRectMethods<crate::DomTypeHolder> for DOMRect {
    /// <https://drafts.csswg.org/geometry/#dom-domrect-domrect>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Fallible<DomRoot<DOMRect>> {
        Ok(DOMRect::new_with_proto(
            cx, global, proto, x, y, width, height,
        ))
    }

    // https://drafts.csswg.org/geometry/#dom-domrect-fromrect
    #[cfg_attr(crown, expect(crown::unrooted_must_root))]
    fn FromRect(cx: &mut JSContext, global: &GlobalScope, other: &DOMRectInit) -> DomRoot<DOMRect> {
        let rect = create_a_domrectreadonly_from_the_dictionary(other);

        reflect_dom_object(
            cx,
            Box::new(Self {
                rect,
                svg_animated_rect: Default::default(),
                svg_rect: false,
                svg_read_only: false,
            }),
            global,
        )
    }

    /// <https://drafts.csswg.org/geometry/#dom-domrect-x>
    fn X(&self, cx: &mut JSContext) -> f64 {
        self.coordinate(cx, 0)
    }

    /// <https://drafts.csswg.org/geometry/#dom-domrect-x>
    fn SetX(&self, cx: &mut JSContext, value: f64) -> ErrorResult {
        self.set_coordinate(cx, 0, value)
    }

    /// <https://drafts.csswg.org/geometry/#dom-domrect-y>
    fn Y(&self, cx: &mut JSContext) -> f64 {
        self.coordinate(cx, 1)
    }

    /// <https://drafts.csswg.org/geometry/#dom-domrect-y>
    fn SetY(&self, cx: &mut JSContext, value: f64) -> ErrorResult {
        self.set_coordinate(cx, 1, value)
    }

    /// <https://drafts.csswg.org/geometry/#dom-domrect-width>
    fn Width(&self, cx: &mut JSContext) -> f64 {
        self.coordinate(cx, 2)
    }

    /// <https://drafts.csswg.org/geometry/#dom-domrect-width>
    fn SetWidth(&self, cx: &mut JSContext, value: f64) -> ErrorResult {
        self.set_coordinate(cx, 2, value)
    }

    /// <https://drafts.csswg.org/geometry/#dom-domrect-height>
    fn Height(&self, cx: &mut JSContext) -> f64 {
        self.coordinate(cx, 3)
    }

    /// <https://drafts.csswg.org/geometry/#dom-domrect-height>
    fn SetHeight(&self, cx: &mut JSContext, value: f64) -> ErrorResult {
        self.set_coordinate(cx, 3, value)
    }
}

impl Serializable<crate::DomTypeHolder> for DOMRect {
    type Index = DomRectIndex;
    type Data = DomRect;

    fn serialize(&self, _no_gc: &NoGC) -> Result<(DomRectId, Self::Data), ()> {
        let [x, y, width, height] = if let Some(animated) = self.svg_animated_rect.get() {
            if self.svg_read_only {
                animated.animated_rect_without_context()
            } else {
                animated.rect()
            }
        } else {
            [
                self.rect.X(),
                self.rect.Y(),
                self.rect.Width(),
                self.rect.Height(),
            ]
        };
        let serialized = DomRect {
            x,
            y,
            width,
            height,
        };
        Ok((DomRectId::new(), serialized))
    }

    fn deserialize(
        cx: &mut JSContext,
        owner: &GlobalScope,
        serialized: Self::Data,
    ) -> Result<DomRoot<Self>, ()>
    where
        Self: Sized,
    {
        Ok(Self::new(
            cx,
            owner,
            serialized.x,
            serialized.y,
            serialized.width,
            serialized.height,
        ))
    }

    fn serialized_storage<'a>(
        data: StructuredData<'a, '_>,
    ) -> &'a mut Option<FxHashMap<DomRectId, Self::Data>> {
        match data {
            StructuredData::Reader(reader) => &mut reader.rects,
            StructuredData::Writer(writer) => &mut writer.rects,
        }
    }
}
