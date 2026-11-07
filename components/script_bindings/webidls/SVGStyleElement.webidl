/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/styling.html#InterfaceSVGStyleElement
[Exposed=Window]
interface SVGStyleElement : SVGElement {
  [CEReactions] attribute DOMString type;
  [CEReactions] attribute DOMString media;
  [CEReactions] attribute DOMString title;
  attribute boolean disabled;
};

SVGStyleElement includes LinkStyle;
