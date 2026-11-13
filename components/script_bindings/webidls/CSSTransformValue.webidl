/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.css-houdini.org/css-typed-om-1/#transformvalue-objects
[Exposed=Window]
interface CSSTransformValue : CSSStyleValue {
  [Throws] constructor(sequence<CSSTransformComponent> transforms);
  iterable<CSSTransformComponent>;
  readonly attribute unsigned long length;
  getter CSSTransformComponent (unsigned long index);
  [Throws] setter undefined (unsigned long index, CSSTransformComponent val);
  readonly attribute boolean is2D;
  [Throws] DOMMatrix toMatrix();
};
