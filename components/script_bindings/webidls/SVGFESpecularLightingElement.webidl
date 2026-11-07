/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://w3c.github.io/fxtf-drafts/filter-effects-1/#InterfaceSVGFESpecularLightingElement
[Exposed=Window]
interface SVGFESpecularLightingElement : SVGElement {
  [SameObject] readonly attribute SVGAnimatedString in1;
  [SameObject] readonly attribute SVGAnimatedNumber surfaceScale;
  [SameObject] readonly attribute SVGAnimatedNumber specularConstant;
  [SameObject] readonly attribute SVGAnimatedNumber specularExponent;
  [SameObject] readonly attribute SVGAnimatedNumber kernelUnitLengthX;
  [SameObject] readonly attribute SVGAnimatedNumber kernelUnitLengthY;
};

SVGFESpecularLightingElement includes SVGFilterPrimitiveStandardAttributes;
