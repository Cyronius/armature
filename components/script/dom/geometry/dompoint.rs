/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use js::context::{JSContext, NoGC};
use js::rust::HandleObject;
use rustc_hash::FxHashMap;
use script_bindings::reflector::reflect_dom_object_with_proto;
use script_bindings::serializable::Serializable;
use script_bindings::structuredclone::StructuredData;
use servo_base::id::{DomPointId, DomPointIndex};
use servo_constellation_traits::DomPoint;

use crate::dom::bindings::codegen::Bindings::DOMPointBinding::{DOMPointInit, DOMPointMethods};
use crate::dom::bindings::codegen::Bindings::DOMPointReadOnlyBinding::DOMPointReadOnlyMethods;
use crate::dom::bindings::error::Fallible;
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::dompointreadonly::{DOMPointReadOnly, DOMPointWriteMethods};
use crate::dom::globalscope::GlobalScope;
use crate::dom::svg::svgpointlist::SVGPointList;

// http://dev.w3.org/csswg/geometry/Overview.html#dompoint
#[dom_struct]
pub(crate) struct DOMPoint {
    point: DOMPointReadOnly,
    /// For an element of an `SVGPointList` (whose elements the SVG DOM calls `SVGPoint`s),
    /// the list, which reflects an attribute.
    svg_point_list: MutNullableDom<SVGPointList>,
    /// Whether this is an element of a read-only `SVGPointList`, and cannot be changed.
    svg_read_only: Cell<bool>,
}

impl DOMPoint {
    fn new_inherited(x: f64, y: f64, z: f64, w: f64) -> DOMPoint {
        DOMPoint {
            point: DOMPointReadOnly::new_inherited(x, y, z, w),
            svg_point_list: Default::default(),
            svg_read_only: Cell::new(false),
        }
    }

    /// Create an element of `list`, an `SVGPointList`.
    pub(crate) fn new_svg_list_item(
        cx: &mut JSContext,
        global: &GlobalScope,
        x: f64,
        y: f64,
        list: &SVGPointList,
        read_only: bool,
    ) -> DomRoot<DOMPoint> {
        let point = Self::new(cx, global, x, y, 0.0, 1.0);
        point.attach_to_svg_list(list, read_only);
        point
    }

    /// The x coordinate, without synchronizing an `SVGPointList` that this point is an
    /// element of first.
    pub(crate) fn x(&self) -> f64 {
        self.point.X()
    }

    /// The y coordinate, without synchronizing an `SVGPointList` first.
    pub(crate) fn y(&self) -> f64 {
        self.point.Y()
    }

    /// Set the coordinates of an element of an `SVGPointList`, without serializing the list.
    pub(crate) fn set_raw_coordinates(&self, x: f64, y: f64) {
        self.point.SetX(x);
        self.point.SetY(y);
    }

    /// Whether this point is an element of an `SVGPointList`, or read-only, in which case a
    /// list that it is inserted into gets a copy.
    pub(crate) fn is_attached_to_svg_list(&self) -> bool {
        self.svg_point_list.get().is_some() || self.svg_read_only.get()
    }

    pub(crate) fn attach_to_svg_list(&self, list: &SVGPointList, read_only: bool) {
        self.svg_point_list.set(Some(list));
        self.svg_read_only.set(read_only);
    }

    /// <https://svgwg.org/svg2-draft/types.html#TermDetach>
    pub(crate) fn detach_from_svg_list(&self) {
        self.svg_point_list.set(None);
        self.svg_read_only.set(false);
    }

    /// Set a coordinate with `set`, serializing an `SVGPointList` that this point is an
    /// element of.
    fn set_coordinate(&self, cx: &mut JSContext, set: impl FnOnce(&DOMPointReadOnly)) {
        if self.svg_read_only.get() {
            return;
        }
        let list = self.svg_point_list.get();
        if let Some(list) = list.as_ref() {
            list.synchronize(cx);
        }
        set(&self.point);
        if let Some(list) = self.svg_point_list.get() {
            list.commit(cx);
        }
    }

    fn synchronize_svg_list(&self, cx: &mut JSContext) {
        if let Some(list) = self.svg_point_list.get() {
            list.synchronize(cx);
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        x: f64,
        y: f64,
        z: f64,
        w: f64,
    ) -> DomRoot<DOMPoint> {
        Self::new_with_proto(cx, global, None, x, y, z, w)
    }

    fn new_with_proto(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        x: f64,
        y: f64,
        z: f64,
        w: f64,
    ) -> DomRoot<DOMPoint> {
        reflect_dom_object_with_proto(
            cx,
            Box::new(DOMPoint::new_inherited(x, y, z, w)),
            global,
            proto,
        )
    }

    pub(crate) fn new_from_init(
        cx: &mut JSContext,
        global: &GlobalScope,
        p: &DOMPointInit,
    ) -> DomRoot<DOMPoint> {
        DOMPoint::new(cx, global, p.x, p.y, p.z, p.w)
    }
}

impl DOMPointMethods<crate::DomTypeHolder> for DOMPoint {
    /// <https://drafts.csswg.org/geometry/#dom-dompointreadonly-dompointreadonly>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        x: f64,
        y: f64,
        z: f64,
        w: f64,
    ) -> Fallible<DomRoot<DOMPoint>> {
        Ok(DOMPoint::new_with_proto(cx, global, proto, x, y, z, w))
    }

    /// <https://drafts.csswg.org/geometry/#dom-dompoint-frompoint>
    fn FromPoint(cx: &mut JSContext, global: &GlobalScope, init: &DOMPointInit) -> DomRoot<Self> {
        Self::new_from_init(cx, global, init)
    }

    /// <https://dev.w3.org/csswg/geometry/Overview.html#dom-dompointreadonly-x>
    fn X(&self, cx: &mut JSContext) -> f64 {
        self.synchronize_svg_list(cx);
        self.point.X()
    }

    /// <https://dev.w3.org/csswg/geometry/Overview.html#dom-dompointreadonly-x>
    fn SetX(&self, cx: &mut JSContext, value: f64) {
        self.set_coordinate(cx, |point| point.SetX(value));
    }

    /// <https://dev.w3.org/csswg/geometry/Overview.html#dom-dompointreadonly-y>
    fn Y(&self, cx: &mut JSContext) -> f64 {
        self.synchronize_svg_list(cx);
        self.point.Y()
    }

    /// <https://dev.w3.org/csswg/geometry/Overview.html#dom-dompointreadonly-y>
    fn SetY(&self, cx: &mut JSContext, value: f64) {
        self.set_coordinate(cx, |point| point.SetY(value));
    }

    /// <https://dev.w3.org/csswg/geometry/Overview.html#dom-dompointreadonly-z>
    fn Z(&self) -> f64 {
        self.point.Z()
    }

    /// <https://dev.w3.org/csswg/geometry/Overview.html#dom-dompointreadonly-z>
    fn SetZ(&self, value: f64) {
        self.point.SetZ(value);
    }

    /// <https://dev.w3.org/csswg/geometry/Overview.html#dom-dompointreadonly-w>
    fn W(&self) -> f64 {
        self.point.W()
    }

    /// <https://dev.w3.org/csswg/geometry/Overview.html#dom-dompointreadonly-w>
    fn SetW(&self, value: f64) {
        self.point.SetW(value);
    }
}

impl Serializable<crate::DomTypeHolder> for DOMPoint {
    type Index = DomPointIndex;
    type Data = DomPoint;

    fn serialize(&self, _no_gc: &NoGC) -> Result<(DomPointId, Self::Data), ()> {
        let serialized = DomPoint {
            x: self.x(),
            y: self.y(),
            z: self.Z(),
            w: self.W(),
        };
        Ok((DomPointId::new(), serialized))
    }

    fn deserialize(
        cx: &mut JSContext,
        owner: &GlobalScope,
        serialized: Self::Data,
    ) -> Result<DomRoot<Self>, ()> {
        Ok(Self::new(
            cx,
            owner,
            serialized.x,
            serialized.y,
            serialized.z,
            serialized.w,
        ))
    }

    fn serialized_storage<'a>(
        data: StructuredData<'a, '_>,
    ) -> &'a mut Option<FxHashMap<DomPointId, Self::Data>> {
        match data {
            StructuredData::Reader(reader) => &mut reader.points,
            StructuredData::Writer(writer) => &mut writer.points,
        }
    }
}
