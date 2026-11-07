/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.fxtf.org/filter-effects/#InterfaceSVGFECompositeElement
[Exposed=Window]
interface SVGFECompositeElement : SVGElement {
  const unsigned short SVG_FECOMPOSITE_OPERATOR_UNKNOWN = 0;
  const unsigned short SVG_FECOMPOSITE_OPERATOR_OVER = 1;
  const unsigned short SVG_FECOMPOSITE_OPERATOR_IN = 2;
  const unsigned short SVG_FECOMPOSITE_OPERATOR_OUT = 3;
  const unsigned short SVG_FECOMPOSITE_OPERATOR_ATOP = 4;
  const unsigned short SVG_FECOMPOSITE_OPERATOR_XOR = 5;
  const unsigned short SVG_FECOMPOSITE_OPERATOR_ARITHMETIC = 6;

  [SameObject] readonly attribute SVGAnimatedString in1;
  [SameObject] readonly attribute SVGAnimatedString in2;
  [SameObject] readonly attribute SVGAnimatedEnumeration operator;
  [SameObject] readonly attribute SVGAnimatedNumber k1;
  [SameObject] readonly attribute SVGAnimatedNumber k2;
  [SameObject] readonly attribute SVGAnimatedNumber k3;
  [SameObject] readonly attribute SVGAnimatedNumber k4;
};

SVGFECompositeElement includes SVGFilterPrimitiveStandardAttributes;
