/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.css-houdini.org/css-typed-om-1/#transformvalue-objects
[Exposed=Window]
interface CSSSkewX : CSSTransformComponent {
  [Throws] constructor(CSSNumericValue ax);
  [SetterThrows] attribute CSSNumericValue ax;
};
