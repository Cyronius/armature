/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://html.spec.whatwg.org/multipage/#the-pagerevealevent-interface
[Exposed=Window]
interface PageRevealEvent : Event {
  constructor(DOMString type, optional PageRevealEventInit eventInitDict = {});
  // Servo has no view transitions, so this is always null.
  readonly attribute object? viewTransition;
};

dictionary PageRevealEventInit : EventInit {
  object? viewTransition = null;
};
