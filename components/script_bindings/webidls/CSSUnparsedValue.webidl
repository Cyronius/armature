/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.css-houdini.org/css-typed-om-1/#cssunparsedvalue
[Exposed=(Window, Worker, Worklet)]
interface CSSUnparsedValue : CSSStyleValue {
    constructor(sequence<CSSUnparsedSegment> members);
    iterable<CSSUnparsedSegment>;
    readonly attribute unsigned long length;
    getter CSSUnparsedSegment (unsigned long index);
    [Throws] setter undefined (unsigned long index, CSSUnparsedSegment val);
};

typedef (USVString or CSSVariableReferenceValue) CSSUnparsedSegment;
