/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/painting.html#InterfaceSVGMarkerElement
[Exposed=Window]
interface SVGMarkerElement : SVGElement {
  const unsigned short SVG_MARKERUNITS_UNKNOWN = 0;
  const unsigned short SVG_MARKERUNITS_USERSPACEONUSE = 1;
  const unsigned short SVG_MARKERUNITS_STROKEWIDTH = 2;
  const unsigned short SVG_MARKER_ORIENT_UNKNOWN = 0;
  const unsigned short SVG_MARKER_ORIENT_AUTO = 1;
  const unsigned short SVG_MARKER_ORIENT_ANGLE = 2;
  const unsigned short SVG_MARKER_ORIENT_AUTO_START_REVERSE = 3;

  [SameObject] readonly attribute SVGAnimatedLength refX;
  [SameObject] readonly attribute SVGAnimatedLength refY;
  [SameObject] readonly attribute SVGAnimatedEnumeration markerUnits;
  [SameObject] readonly attribute SVGAnimatedLength markerWidth;
  [SameObject] readonly attribute SVGAnimatedLength markerHeight;
  [SameObject] readonly attribute SVGAnimatedEnumeration orientType;
  [SameObject] readonly attribute SVGAnimatedAngle orientAngle;
  attribute DOMString orient;

  undefined setOrientToAuto();
  undefined setOrientToAngle(SVGAngle angle);
};

SVGMarkerElement includes SVGFitToViewBox;
