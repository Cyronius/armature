/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/linking.html#InterfaceSVGAElement
[Exposed=Window]
interface SVGAElement : SVGGraphicsElement {
  [SameObject] readonly attribute SVGAnimatedString target;
  [CEReactions] attribute DOMString download;
  [CEReactions] attribute USVString ping;
  [CEReactions] attribute DOMString rel;
  [SameObject, PutForwards=value] readonly attribute DOMTokenList relList;
  [CEReactions] attribute DOMString hreflang;
  [CEReactions] attribute DOMString type;

  [CEReactions] attribute DOMString referrerPolicy;

  // The members of HTMLHyperlinkElementUtils other than href, which is the href of
  // SVGURIReference.
  // <https://svgwg.org/svg2-draft/linking.html#InterfaceSVGAElement>
  readonly attribute USVString origin;
  [CEReactions] attribute USVString protocol;
  [CEReactions] attribute USVString username;
  [CEReactions] attribute USVString password;
  [CEReactions] attribute USVString host;
  [CEReactions] attribute USVString hostname;
  [CEReactions] attribute USVString port;
  [CEReactions] attribute USVString pathname;
  [CEReactions] attribute USVString search;
  [CEReactions] attribute USVString hash;
};

SVGAElement includes SVGURIReference;
// SVGAElement includes HTMLHyperlinkElementUtils;
