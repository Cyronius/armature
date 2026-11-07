/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.fxtf.org/filter-effects/#InterfaceSVGFEConvolveMatrixElement
[Exposed=Window]
interface SVGFEConvolveMatrixElement : SVGElement {
  const unsigned short SVG_EDGEMODE_UNKNOWN = 0;
  const unsigned short SVG_EDGEMODE_DUPLICATE = 1;
  const unsigned short SVG_EDGEMODE_WRAP = 2;
  const unsigned short SVG_EDGEMODE_NONE = 3;

  [SameObject] readonly attribute SVGAnimatedString in1;
  [SameObject] readonly attribute SVGAnimatedInteger orderX;
  [SameObject] readonly attribute SVGAnimatedInteger orderY;
  [SameObject] readonly attribute SVGAnimatedNumber divisor;
  [SameObject] readonly attribute SVGAnimatedNumber bias;
  [SameObject] readonly attribute SVGAnimatedInteger targetX;
  [SameObject] readonly attribute SVGAnimatedInteger targetY;
  [SameObject] readonly attribute SVGAnimatedEnumeration edgeMode;
  [SameObject] readonly attribute SVGAnimatedNumber kernelUnitLengthX;
  [SameObject] readonly attribute SVGAnimatedNumber kernelUnitLengthY;
  [SameObject] readonly attribute SVGAnimatedBoolean preserveAlpha;
  [SameObject] readonly attribute SVGAnimatedNumberList kernelMatrix;
};

SVGFEConvolveMatrixElement includes SVGFilterPrimitiveStandardAttributes;
