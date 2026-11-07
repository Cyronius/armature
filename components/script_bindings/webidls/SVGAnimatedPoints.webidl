/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/shapes.html#InterfaceSVGAnimatedPoints
interface mixin SVGAnimatedPoints {
  [SameObject] readonly attribute SVGPointList points;
  [SameObject] readonly attribute SVGPointList animatedPoints;
};
