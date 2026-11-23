/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://html.spec.whatwg.org/multipage/#workerglobalscope
[Abstract, Exposed=Worker]
interface WorkerGlobalScope : GlobalScope {
  [BinaryName="Self_"] readonly attribute WorkerGlobalScope self;
  readonly attribute WorkerLocation location;
  readonly attribute WorkerNavigator navigator;
  [Throws] undefined importScripts((TrustedScriptURL or USVString)... urls);

  attribute OnErrorEventHandler onerror;
  attribute EventHandler onlanguagechange;
  attribute EventHandler onoffline;
  attribute EventHandler ononline;
  attribute EventHandler onrejectionhandled;
  attribute EventHandler onunhandledrejection;

  readonly attribute DOMString crossOriginEmbedderPolicy;
};

// https://drafts.csswg.org/css-font-loading/#font-face-source
partial interface WorkerGlobalScope {
  [Pref="dom_fontface_enabled"]
  readonly attribute FontFaceSet fonts;
};
