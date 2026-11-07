/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use script_bindings::reflector::Reflector;

use crate::dom::bindings::codegen::Bindings::SVGUnitTypesBinding::SVGUnitTypesMethods;

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGUnitTypes>
///
/// The interface only has constants, and no object implements it.
#[dom_struct]
pub(crate) struct SVGUnitTypes {
    reflector_: Reflector,
}

impl SVGUnitTypesMethods<crate::DomTypeHolder> for SVGUnitTypes {}

/// The keywords of attributes whose values are `SVGUnitTypes` constants.
pub(crate) const UNIT_TYPE_KEYWORDS: &[(&str, u16)] =
    &[("userSpaceOnUse", 1), ("objectBoundingBox", 2)];
pub(crate) const SVG_UNIT_TYPE_USERSPACEONUSE: u16 = 1;
pub(crate) const SVG_UNIT_TYPE_OBJECTBOUNDINGBOX: u16 = 2;
