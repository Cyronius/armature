/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.fxtf.org/filter-effects/#InterfaceSVGFEOffsetElement
[Exposed=Window]
interface SVGFEOffsetElement : SVGElement {
  [SameObject] readonly attribute SVGAnimatedString in1;
  [SameObject] readonly attribute SVGAnimatedNumber dx;
  [SameObject] readonly attribute SVGAnimatedNumber dy;
};

SVGFEOffsetElement includes SVGFilterPrimitiveStandardAttributes;
