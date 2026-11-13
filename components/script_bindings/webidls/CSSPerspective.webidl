/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.css-houdini.org/css-typed-om-1/#transformvalue-objects
typedef (DOMString or CSSKeywordValue) CSSKeywordish;
typedef (CSSNumericValue or CSSKeywordish) CSSPerspectiveValue;

[Exposed=Window]
interface CSSPerspective : CSSTransformComponent {
  [Throws] constructor(CSSPerspectiveValue length);
  [SetterThrows] attribute CSSPerspectiveValue length;
};
