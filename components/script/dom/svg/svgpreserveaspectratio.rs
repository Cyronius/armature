/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGPreserveAspectRatioBinding::SVGPreserveAspectRatioMethods;
use crate::dom::bindings::error::{Error, ErrorResult};
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::svg::svganimatedpreserveaspectratio::{
    SVG_MEETORSLICE_MEET, SVG_MEETORSLICE_SLICE, SVG_PRESERVEASPECTRATIO_XMAXYMAX,
    SVGAnimatedPreserveAspectRatio,
};

/// <https://svgwg.org/svg2-draft/coords.html#InterfaceSVGPreserveAspectRatio>
///
/// The base or animated value of an `SVGAnimatedPreserveAspectRatio`, which reflects the
/// `preserveAspectRatio` attribute.
#[dom_struct]
pub(crate) struct SVGPreserveAspectRatio {
    reflector_: Reflector,
    animated: Dom<SVGAnimatedPreserveAspectRatio>,
    read_only: bool,
}

impl SVGPreserveAspectRatio {
    pub(crate) fn new(
        cx: &mut JSContext,
        animated: &SVGAnimatedPreserveAspectRatio,
        read_only: bool,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                animated: Dom::from_ref(animated),
                read_only,
            }),
            &*animated.global(),
        )
    }

    fn check_writable(&self) -> ErrorResult {
        if self.read_only {
            return Err(Error::NoModificationAllowed(Some(
                "This SVGPreserveAspectRatio is read-only".into(),
            )));
        }
        Ok(())
    }
}

impl SVGPreserveAspectRatioMethods<crate::DomTypeHolder> for SVGPreserveAspectRatio {
    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGPreserveAspectRatio__align>
    fn Align(&self) -> u16 {
        self.animated.value().0
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGPreserveAspectRatio__align>
    fn SetAlign(&self, cx: &mut JSContext, align: u16) -> ErrorResult {
        self.check_writable()?;
        if align == 0 || align > SVG_PRESERVEASPECTRATIO_XMAXYMAX {
            return Err(Error::NotSupported(Some("Invalid alignment".into())));
        }
        let (_, meet_or_slice) = self.animated.value();
        self.animated.set_value(cx, align, meet_or_slice);
        Ok(())
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGPreserveAspectRatio__meetOrSlice>
    fn MeetOrSlice(&self) -> u16 {
        self.animated.value().1
    }

    /// <https://svgwg.org/svg2-draft/coords.html#__svg__SVGPreserveAspectRatio__meetOrSlice>
    fn SetMeetOrSlice(&self, cx: &mut JSContext, meet_or_slice: u16) -> ErrorResult {
        self.check_writable()?;
        if meet_or_slice != SVG_MEETORSLICE_MEET && meet_or_slice != SVG_MEETORSLICE_SLICE {
            return Err(Error::NotSupported(Some(
                "Invalid meet-or-slice type".into(),
            )));
        }
        let (align, _) = self.animated.value();
        self.animated.set_value(cx, align, meet_or_slice);
        Ok(())
    }
}
