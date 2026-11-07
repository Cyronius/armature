/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedNumberList
[Exposed=Window]
interface SVGAnimatedNumberList {
  [SameObject] readonly attribute SVGNumberList baseVal;
  [SameObject] readonly attribute SVGNumberList animVal;
};
