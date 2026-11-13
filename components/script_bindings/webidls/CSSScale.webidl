/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.css-houdini.org/css-typed-om-1/#transformvalue-objects
[Exposed=Window]
interface CSSScale : CSSTransformComponent {
  [Throws] constructor(CSSNumberish x, CSSNumberish y, optional CSSNumberish z);
  [SetterThrows] attribute CSSNumberish x;
  [SetterThrows] attribute CSSNumberish y;
  [SetterThrows] attribute CSSNumberish z;
};
