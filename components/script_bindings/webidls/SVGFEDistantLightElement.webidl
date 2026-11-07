/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://w3c.github.io/fxtf-drafts/filter-effects-1/#InterfaceSVGFEDistantLightElement
[Exposed=Window]
interface SVGFEDistantLightElement : SVGElement {
  [SameObject] readonly attribute SVGAnimatedNumber azimuth;
  [SameObject] readonly attribute SVGAnimatedNumber elevation;
};
