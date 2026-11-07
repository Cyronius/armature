/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/coords.html#InterfaceSVGAnimatedTransformList
[Exposed=Window]
interface SVGAnimatedTransformList {
  [SameObject] readonly attribute SVGTransformList baseVal;
  [SameObject] readonly attribute SVGTransformList animVal;
};
