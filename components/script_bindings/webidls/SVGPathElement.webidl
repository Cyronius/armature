/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/paths.html#InterfaceSVGPathElement
dictionary SVGPathSegment {
  required DOMString type;
  required sequence<unrestricted double> values;
};

dictionary SVGPathDataOptions {
  boolean normalize = false;
};

[Exposed=Window]
interface SVGPathElement : SVGGeometryElement {
  sequence<SVGPathSegment> getPathData(optional SVGPathDataOptions options = {});
  undefined setPathData(sequence<SVGPathSegment> pathData);
  SVGPathSegment? getPathSegmentAtLength(double distance);
};
