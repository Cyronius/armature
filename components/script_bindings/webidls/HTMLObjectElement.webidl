/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://html.spec.whatwg.org/multipage/#htmlobjectelement
[Exposed=Window]
interface HTMLObjectElement : HTMLElement {
  [HTMLConstructor] constructor();

  [CEReactions]
          attribute USVString data;
  [CEReactions]
           attribute DOMString type;
  // [CEReactions]
  //         attribute boolean typeMustMatch;
  [CEReactions]
          attribute DOMString name;
  readonly attribute HTMLFormElement? form;
  [CEReactions]
          attribute DOMString width;
  [CEReactions]
          attribute DOMString height;
  //readonly attribute Document? contentDocument;
  //readonly attribute WindowProxy? contentWindow;

  readonly attribute boolean willValidate;
  readonly attribute ValidityState validity;
  readonly attribute DOMString validationMessage;
  boolean checkValidity();
  boolean reportValidity();
  undefined setCustomValidity(DOMString error);

  //legacycaller any (any... arguments);

  // also has obsolete members
};

// https://html.spec.whatwg.org/multipage/#HTMLObjectElement-partial
partial interface HTMLObjectElement {
  [CEReactions] attribute DOMString align;
  [CEReactions] attribute DOMString archive;
  [CEReactions] attribute DOMString code;
  [CEReactions] attribute boolean declare;
  [CEReactions] attribute unsigned long hspace;
  [CEReactions] attribute DOMString standby;
  [CEReactions] attribute unsigned long vspace;
  [CEReactions] attribute USVString codeBase;
  [CEReactions] attribute DOMString codeType;
  [CEReactions]
  attribute DOMString useMap;

  [CEReactions] attribute [LegacyNullToEmptyString] DOMString border;
};
