/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};
use script_bindings::str::DOMString;

use crate::dom::bindings::codegen::Bindings::SVGAngleBinding::SVGAngleMethods;
use crate::dom::bindings::error::{Error, ErrorResult};
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;
use crate::dom::node::NodeTraits;
use crate::dom::svg::svganimatedangle::SVGAnimatedAngle;
use crate::dom::svg::svgtypes::{SVG_ANGLETYPE_UNSPECIFIED, SVGAngleValue};

/// Where the value of an [`SVGAngle`] lives.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
enum SVGAngleStorage {
    /// An angle that is not reflected, like one from `createSVGAngle()`.
    Detached(Cell<SVGAngleValue>),
    /// The base or animated value of an `SVGAnimatedAngle`.
    Reflected(Dom<SVGAnimatedAngle>),
}

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAngle>
#[dom_struct]
pub(crate) struct SVGAngle {
    reflector_: Reflector,
    storage: SVGAngleStorage,
    read_only: bool,
}

impl SVGAngle {
    /// Create an angle that is not reflected, with the value `0`.
    pub(crate) fn new_detached(cx: &mut JSContext, global: &GlobalScope) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                storage: SVGAngleStorage::Detached(Cell::new(SVGAngleValue::new(
                    0.0,
                    SVG_ANGLETYPE_UNSPECIFIED,
                ))),
                read_only: false,
            }),
            global,
        )
    }

    /// Create the base value, or with `read_only` the animated value, of `animated`.
    pub(crate) fn new_reflected(
        cx: &mut JSContext,
        animated: &SVGAnimatedAngle,
        read_only: bool,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                storage: SVGAngleStorage::Reflected(Dom::from_ref(animated)),
                read_only,
            }),
            &*animated.element().owner_window(),
        )
    }

    pub(crate) fn angle(&self, cx: &mut JSContext) -> SVGAngleValue {
        match &self.storage {
            SVGAngleStorage::Detached(value) => value.get(),
            SVGAngleStorage::Reflected(animated) => {
                if self.read_only {
                    animated.animated_value(cx)
                } else {
                    animated.base_value()
                }
            },
        }
    }

    fn set_angle(&self, cx: &mut JSContext, angle: SVGAngleValue) -> ErrorResult {
        if self.read_only {
            return Err(Error::NoModificationAllowed(Some(
                "This SVGAngle is read-only".into(),
            )));
        }
        match &self.storage {
            SVGAngleStorage::Detached(value) => value.set(angle),
            SVGAngleStorage::Reflected(animated) => animated.set_base_value(cx, angle),
        }
        Ok(())
    }

    fn check_unit_type(&self, unit_type: u16) -> ErrorResult {
        if self.read_only {
            return Err(Error::NoModificationAllowed(Some(
                "This SVGAngle is read-only".into(),
            )));
        }
        if !SVGAngleValue::is_valid_unit_type(unit_type) {
            return Err(Error::NotSupported(Some("Invalid angle unit type".into())));
        }
        Ok(())
    }
}

impl SVGAngleMethods<crate::DomTypeHolder> for SVGAngle {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAngle__unitType>
    fn UnitType(&self, cx: &mut JSContext) -> u16 {
        self.angle(cx).dom_unit_type()
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAngle__value>
    fn Value(&self, cx: &mut JSContext) -> Finite<f32> {
        Finite::wrap(self.angle(cx).to_degrees())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAngle__value>
    fn SetValue(&self, cx: &mut JSContext, value: Finite<f32>) -> ErrorResult {
        let unit_type = self.angle(cx).unit_type;
        self.set_angle(cx, SVGAngleValue::from_degrees(*value, unit_type))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAngle__valueInSpecifiedUnits>
    fn ValueInSpecifiedUnits(&self, cx: &mut JSContext) -> Finite<f32> {
        Finite::wrap(self.angle(cx).value)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAngle__valueInSpecifiedUnits>
    fn SetValueInSpecifiedUnits(&self, cx: &mut JSContext, value: Finite<f32>) -> ErrorResult {
        let unit_type = self.angle(cx).unit_type;
        self.set_angle(cx, SVGAngleValue::new(*value, unit_type))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAngle__valueAsString>
    fn ValueAsString(&self, cx: &mut JSContext) -> DOMString {
        DOMString::from(self.angle(cx).to_string())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAngle__valueAsString>
    fn SetValueAsString(&self, cx: &mut JSContext, value: DOMString) -> ErrorResult {
        if self.read_only {
            return Err(Error::NoModificationAllowed(Some(
                "This SVGAngle is read-only".into(),
            )));
        }
        let angle = SVGAngleValue::parse(&value.str())
            .ok_or_else(|| Error::Syntax(Some("The value is not a valid angle".into())))?;
        self.set_angle(cx, angle)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAngle__newValueSpecifiedUnits>
    fn NewValueSpecifiedUnits(
        &self,
        cx: &mut JSContext,
        unit_type: u16,
        value: Finite<f32>,
    ) -> ErrorResult {
        self.check_unit_type(unit_type)?;
        self.set_angle(cx, SVGAngleValue::new(*value, unit_type))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAngle__convertToSpecifiedUnits>
    fn ConvertToSpecifiedUnits(&self, cx: &mut JSContext, unit_type: u16) -> ErrorResult {
        self.check_unit_type(unit_type)?;
        let degrees = self.angle(cx).to_degrees();
        self.set_angle(cx, SVGAngleValue::from_degrees(degrees, unit_type))
    }
}
