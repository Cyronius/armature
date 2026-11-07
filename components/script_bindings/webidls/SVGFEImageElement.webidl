/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://w3c.github.io/fxtf-drafts/filter-effects-1/#InterfaceSVGFEImageElement
[Exposed=Window]
interface SVGFEImageElement : SVGElement {
  [SameObject] readonly attribute SVGAnimatedPreserveAspectRatio preserveAspectRatio;
  [SameObject] readonly attribute SVGAnimatedString crossOrigin;
};

SVGFEImageElement includes SVGFilterPrimitiveStandardAttributes;

SVGFEImageElement includes SVGURIReference;
