/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedAngle
[Exposed=Window]
interface SVGAnimatedAngle {
  [SameObject] readonly attribute SVGAngle baseVal;
  [SameObject] readonly attribute SVGAngle animVal;
};
