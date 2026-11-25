/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://w3c.github.io/clipboard-apis/#clipboardchangeevent
[Exposed=Window, Pref="dom_async_clipboard_enabled"]
interface ClipboardChangeEvent : Event {
  [Throws] constructor(DOMString type, optional ClipboardChangeEventInit eventInitDict = {});
  readonly attribute /* FrozenArray<DOMString> */ any types;
  readonly attribute /* bigint */ any changeId;
};

dictionary ClipboardChangeEventInit : EventInit {
  sequence<DOMString> types = [];
  // The binding does not yet support WebIDL bigint. Convert in the constructor.
  any changeId;
};
