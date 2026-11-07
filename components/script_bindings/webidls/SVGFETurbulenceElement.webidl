/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.fxtf.org/filter-effects/#InterfaceSVGFETurbulenceElement
[Exposed=Window]
interface SVGFETurbulenceElement : SVGElement {
  const unsigned short SVG_TURBULENCE_TYPE_UNKNOWN = 0;
  const unsigned short SVG_TURBULENCE_TYPE_FRACTALNOISE = 1;
  const unsigned short SVG_TURBULENCE_TYPE_TURBULENCE = 2;
  const unsigned short SVG_STITCHTYPE_UNKNOWN = 0;
  const unsigned short SVG_STITCHTYPE_STITCH = 1;
  const unsigned short SVG_STITCHTYPE_NOSTITCH = 2;

  [SameObject] readonly attribute SVGAnimatedNumber baseFrequencyX;
  [SameObject] readonly attribute SVGAnimatedNumber baseFrequencyY;
  [SameObject] readonly attribute SVGAnimatedInteger numOctaves;
  [SameObject] readonly attribute SVGAnimatedNumber seed;
  [SameObject] readonly attribute SVGAnimatedEnumeration stitchTiles;
  [SameObject] readonly attribute SVGAnimatedEnumeration type;
};

SVGFETurbulenceElement includes SVGFilterPrimitiveStandardAttributes;
