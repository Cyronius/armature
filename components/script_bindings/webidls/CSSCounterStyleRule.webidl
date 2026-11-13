/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://drafts.csswg.org/css-counter-styles-3/#the-csscounterstylerule-interface

[Exposed=Window]
interface CSSCounterStyleRule : CSSRule {
  attribute CSSOMString name;
  attribute CSSOMString system;
  attribute CSSOMString symbols;
  attribute CSSOMString additiveSymbols;
  attribute CSSOMString negative;
  attribute CSSOMString prefix;
  attribute CSSOMString suffix;
  attribute CSSOMString range;
  attribute CSSOMString pad;
  attribute CSSOMString speakAs;
  attribute CSSOMString fallback;
};
