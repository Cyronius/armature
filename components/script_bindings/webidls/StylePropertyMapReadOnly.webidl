/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.css-houdini.org/css-typed-om-1/#stylepropertymapreadonly
[Exposed=(Window, Worker, Worklet)]
interface StylePropertyMapReadOnly {
    [Throws] any get(DOMString property);
    [Throws] sequence<CSSStyleValue> getAll(DOMString property);
    [Throws] boolean has(DOMString property);
    readonly attribute unsigned long size;
    // iterable<DOMString, (CSSStyleValue or sequence<CSSStyleValue>)>;
    sequence<DOMString> getProperties();
    // https://github.com/w3c/css-houdini-drafts/issues/268
    // stringifier;
};
