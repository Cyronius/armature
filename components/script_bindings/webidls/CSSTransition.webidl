/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.csswg.org/css-transitions-2/#the-CSSTransition-interface

[Exposed=Window, Pref="dom_web_animations_enabled"]
interface CSSTransition : Animation {
  readonly attribute CSSOMString transitionProperty;
};
