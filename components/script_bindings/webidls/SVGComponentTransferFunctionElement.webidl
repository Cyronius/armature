/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.fxtf.org/filter-effects/#InterfaceSVGComponentTransferFunctionElement
[Exposed=Window, Abstract]
interface SVGComponentTransferFunctionElement : SVGElement {
  const unsigned short SVG_FECOMPONENTTRANSFER_TYPE_UNKNOWN = 0;
  const unsigned short SVG_FECOMPONENTTRANSFER_TYPE_IDENTITY = 1;
  const unsigned short SVG_FECOMPONENTTRANSFER_TYPE_TABLE = 2;
  const unsigned short SVG_FECOMPONENTTRANSFER_TYPE_DISCRETE = 3;
  const unsigned short SVG_FECOMPONENTTRANSFER_TYPE_LINEAR = 4;
  const unsigned short SVG_FECOMPONENTTRANSFER_TYPE_GAMMA = 5;

  [SameObject] readonly attribute SVGAnimatedEnumeration type;
  [SameObject] readonly attribute SVGAnimatedNumber slope;
  [SameObject] readonly attribute SVGAnimatedNumber intercept;
  [SameObject] readonly attribute SVGAnimatedNumber amplitude;
  [SameObject] readonly attribute SVGAnimatedNumber exponent;
  [SameObject] readonly attribute SVGAnimatedNumber offset;
  [SameObject] readonly attribute SVGAnimatedNumberList tableValues;
};
