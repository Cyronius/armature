/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.fxtf.org/filter-effects/#InterfaceSVGFESpotLightElement
[Exposed=Window]
interface SVGFESpotLightElement : SVGElement {
  [SameObject] readonly attribute SVGAnimatedNumber x;
  [SameObject] readonly attribute SVGAnimatedNumber y;
  [SameObject] readonly attribute SVGAnimatedNumber z;
  [SameObject] readonly attribute SVGAnimatedNumber pointsAtX;
  [SameObject] readonly attribute SVGAnimatedNumber pointsAtY;
  [SameObject] readonly attribute SVGAnimatedNumber pointsAtZ;
  [SameObject] readonly attribute SVGAnimatedNumber specularExponent;
  [SameObject] readonly attribute SVGAnimatedNumber limitingConeAngle;
};
