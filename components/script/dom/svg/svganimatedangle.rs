/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, local_name};
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedAngleBinding::SVGAnimatedAngleMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::element::Element;
use crate::dom::node::NodeTraits;
use crate::dom::svg::svgangle::SVGAngle;
use crate::dom::svg::svganimationelement::{SVGAnimationElement, trim_one_trailing_semicolon};
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgtypes::{SVG_ANGLETYPE_UNSPECIFIED, SVGAngleValue};

#[derive(Clone, Copy)]
pub(crate) enum SVGOrientValue {
    Angle(SVGAngleValue),
    Auto,
    AutoStartReverse,
}

impl SVGOrientValue {
    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "auto" => Some(Self::Auto),
            "auto-start-reverse" => Some(Self::AutoStartReverse),
            value => SVGAngleValue::parse(value).map(Self::Angle),
        }
    }

    pub(crate) fn angle(self) -> SVGAngleValue {
        match self {
            Self::Angle(angle) => angle,
            Self::Auto | Self::AutoStartReverse => {
                SVGAngleValue::new(0.0, SVG_ANGLETYPE_UNSPECIFIED)
            },
        }
    }

    pub(crate) fn orient_type(self) -> u16 {
        match self {
            Self::Auto => 1,
            Self::Angle(_) => 2,
            Self::AutoStartReverse => 3,
        }
    }
}

fn interpolate_orient(
    animation: &SVGAnimationElement,
    start: SVGOrientValue,
    end: SVGOrientValue,
    progress: f32,
) -> Option<SVGOrientValue> {
    if let (SVGOrientValue::Angle(start), SVGOrientValue::Angle(end)) = (start, end) {
        let (_, fraction) = animation.sample_value_interval(2, progress)?;
        let degrees = start.to_degrees() + (end.to_degrees() - start.to_degrees()) * fraction;
        let unit_type = if fraction <= 0.5 {
            start.unit_type
        } else {
            end.unit_type
        };
        return Some(SVGOrientValue::Angle(SVGAngleValue::from_degrees(
            degrees, unit_type,
        )));
    }
    Some(if animation.sample_discrete_index(2)? == 0 {
        start
    } else {
        end
    })
}

pub(crate) fn animated_orient_value(cx: &mut JSContext, element: &SVGElement) -> SVGOrientValue {
    let mut result = element
        .svg_attribute(&local_name!("orient"))
        .as_deref()
        .and_then(SVGOrientValue::parse)
        .unwrap_or(SVGOrientValue::Angle(SVGAngleValue::new(
            0.0,
            SVG_ANGLETYPE_UNSPECIFIED,
        )));
    for animation in element
        .owner_document()
        .smil_animations_targeting(cx, element)
    {
        if animation.upcast::<Element>().local_name() != &local_name!("animate") {
            continue;
        }
        let source = animation.upcast::<SVGElement>();
        if source
            .svg_attribute(&local_name!("attributeName"))
            .as_deref() !=
            Some("orient")
        {
            continue;
        }
        let Some(progress) = animation.sample_progress() else {
            continue;
        };
        if let Some(values) = source.svg_attribute(&local_name!("values")) {
            let values: Option<Vec<_>> = trim_one_trailing_semicolon(&values)
                .split(';')
                .map(SVGOrientValue::parse)
                .collect();
            let Some(values) = values.filter(|values| !values.is_empty()) else {
                continue;
            };
            if values.len() == 1 {
                result = values[0];
            } else if values
                .iter()
                .any(|value| !matches!(value, SVGOrientValue::Angle(_)))
            {
                if let Some(index) = animation.sample_discrete_index(values.len()) {
                    result = values[index];
                }
            } else if let Some((index, fraction)) =
                animation.sample_value_interval(values.len(), progress)
            {
                let start = values[index].angle();
                let end = values[index + 1].angle();
                let degrees =
                    start.to_degrees() + (end.to_degrees() - start.to_degrees()) * fraction;
                let unit_type = if fraction <= 0.5 {
                    start.unit_type
                } else {
                    end.unit_type
                };
                result = SVGOrientValue::Angle(SVGAngleValue::from_degrees(degrees, unit_type));
            }
            continue;
        }
        let from = source
            .svg_attribute(&local_name!("from"))
            .as_deref()
            .and_then(SVGOrientValue::parse);
        let to = source
            .svg_attribute(&local_name!("to"))
            .as_deref()
            .and_then(SVGOrientValue::parse);
        let by = source
            .svg_attribute(&local_name!("by"))
            .as_deref()
            .and_then(SVGAngleValue::parse);
        let start = from.unwrap_or(result);
        let end = to.or_else(|| match (start, by) {
            (SVGOrientValue::Angle(start), Some(by)) => Some(SVGOrientValue::Angle(
                SVGAngleValue::from_degrees(start.to_degrees() + by.to_degrees(), start.unit_type),
            )),
            _ => None,
        });
        if let Some(end) = end &&
            let Some(value) = interpolate_orient(&animation, start, end, progress)
        {
            result = value;
        }
    }
    result
}

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedAngle>
///
/// Reflects an attribute whose value may be an angle, like the `orient` attribute of a
/// marker, which may also be `auto`. When the attribute is not an angle, the angle is zero.
#[dom_struct]
pub(crate) struct SVGAnimatedAngle {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    base_val: MutNullableDom<SVGAngle>,
    anim_val: MutNullableDom<SVGAngle>,
}

impl SVGAnimatedAngle {
    pub(crate) fn new(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: LocalName,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                element: Dom::from_ref(element),
                attribute,
                base_val: Default::default(),
                anim_val: Default::default(),
            }),
            &*element.owner_window(),
        )
    }

    pub(crate) fn element(&self) -> &SVGElement {
        &self.element
    }

    /// The base value, parsed from the attribute.
    pub(crate) fn base_value(&self) -> SVGAngleValue {
        self.element
            .svg_attribute(&self.attribute)
            .and_then(|value| SVGAngleValue::parse(&value))
            .unwrap_or(SVGAngleValue::new(0.0, SVG_ANGLETYPE_UNSPECIFIED))
    }

    pub(crate) fn animated_value(&self, cx: &mut JSContext) -> SVGAngleValue {
        if self.attribute == local_name!("orient") {
            animated_orient_value(cx, &self.element).angle()
        } else {
            self.base_value()
        }
    }

    /// Set the base value, by setting the attribute.
    pub(crate) fn set_base_value(&self, cx: &mut JSContext, value: SVGAngleValue) {
        self.element
            .set_svg_attribute(cx, &self.attribute, value.to_string());
    }
}

impl SVGAnimatedAngleMethods<crate::DomTypeHolder> for SVGAnimatedAngle {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedAngle__baseVal>
    fn BaseVal(&self, cx: &mut JSContext) -> DomRoot<SVGAngle> {
        if let Some(angle) = self.base_val.get() {
            return angle;
        }
        let angle = SVGAngle::new_reflected(cx, self, false);
        self.base_val.set(Some(&angle));
        angle
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedAngle__animVal>
    fn AnimVal(&self, cx: &mut JSContext) -> DomRoot<SVGAngle> {
        if let Some(angle) = self.anim_val.get() {
            return angle;
        }
        let angle = SVGAngle::new_reflected(cx, self, true);
        self.anim_val.set(Some(&angle));
        angle
    }
}
