/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://html.spec.whatwg.org/multipage/#the-body-element
[Exposed=Window]
interface HTMLBodyElement : HTMLElement {
  [HTMLConstructor] constructor();

  // also has obsolete members
};
HTMLBodyElement includes WindowEventHandlers;

// https://html.spec.whatwg.org/multipage/#HTMLBodyElement-partial
partial interface HTMLBodyElement {
  [CEReactions] attribute [LegacyNullToEmptyString] DOMString text;

  [CEReactions] attribute [LegacyNullToEmptyString] DOMString link;

  [CEReactions] attribute [LegacyNullToEmptyString] DOMString vLink;

  [CEReactions] attribute [LegacyNullToEmptyString] DOMString aLink;

  [CEReactions] attribute [LegacyNullToEmptyString] DOMString bgColor;
  [CEReactions] attribute DOMString background;
};
