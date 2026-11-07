/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.fxtf.org/filter-effects/#InterfaceSVGFEDropShadowElement
[Exposed=Window]
interface SVGFEDropShadowElement : SVGElement {
  [SameObject] readonly attribute SVGAnimatedString in1;
  [SameObject] readonly attribute SVGAnimatedNumber dx;
  [SameObject] readonly attribute SVGAnimatedNumber dy;
  [SameObject] readonly attribute SVGAnimatedNumber stdDeviationX;
  [SameObject] readonly attribute SVGAnimatedNumber stdDeviationY;

  undefined setStdDeviation(float stdDeviationX, float stdDeviationY);
};

SVGFEDropShadowElement includes SVGFilterPrimitiveStandardAttributes;
