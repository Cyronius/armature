/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/text.html#InterfaceSVGTextPathElement
[Exposed=Window]
interface SVGTextPathElement : SVGTextContentElement {
  const unsigned short TEXTPATH_METHODTYPE_UNKNOWN = 0;
  const unsigned short TEXTPATH_METHODTYPE_ALIGN = 1;
  const unsigned short TEXTPATH_METHODTYPE_STRETCH = 2;
  const unsigned short TEXTPATH_SPACINGTYPE_UNKNOWN = 0;
  const unsigned short TEXTPATH_SPACINGTYPE_AUTO = 1;
  const unsigned short TEXTPATH_SPACINGTYPE_EXACT = 2;
  const unsigned short TEXTPATH_SIDETYPE_UNKNOWN = 0;
  const unsigned short TEXTPATH_SIDETYPE_LEFT = 1;
  const unsigned short TEXTPATH_SIDETYPE_RIGHT = 2;

  [SameObject] readonly attribute SVGAnimatedLength startOffset;
  [SameObject] readonly attribute SVGAnimatedEnumeration method;
  [SameObject] readonly attribute SVGAnimatedEnumeration spacing;
  [SameObject] readonly attribute SVGAnimatedEnumeration side;
};

SVGTextPathElement includes SVGURIReference;
