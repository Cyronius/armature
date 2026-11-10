/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.css-houdini.org/css-typed-om-1/#cssnumericvalue
enum CSSNumericBaseType { "length", "angle", "time", "frequency", "resolution", "flex", "percent" };

dictionary CSSNumericType {
    long length;
    long angle;
    long time;
    long frequency;
    long resolution;
    long flex;
    long percent;
    CSSNumericBaseType percentHint;
};

[Exposed=(Window, Worker, Worklet)]
interface CSSNumericValue : CSSStyleValue {
    [Throws] CSSUnitValue to(USVString unit);
    CSSNumericType type();
    [Exposed=Window, Throws] static CSSNumericValue parse(USVString cssText);
};
