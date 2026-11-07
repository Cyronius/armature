/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.fxtf.org/filter-effects/#InterfaceSVGFEDisplacementMapElement
[Exposed=Window]
interface SVGFEDisplacementMapElement : SVGElement {
  const unsigned short SVG_CHANNEL_UNKNOWN = 0;
  const unsigned short SVG_CHANNEL_R = 1;
  const unsigned short SVG_CHANNEL_G = 2;
  const unsigned short SVG_CHANNEL_B = 3;
  const unsigned short SVG_CHANNEL_A = 4;

  [SameObject] readonly attribute SVGAnimatedString in1;
  [SameObject] readonly attribute SVGAnimatedString in2;
  [SameObject] readonly attribute SVGAnimatedNumber scale;
  [SameObject] readonly attribute SVGAnimatedEnumeration xChannelSelector;
  [SameObject] readonly attribute SVGAnimatedEnumeration yChannelSelector;
};

SVGFEDisplacementMapElement includes SVGFilterPrimitiveStandardAttributes;
