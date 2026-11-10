/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
/*
 * The origin of this IDL file is
 * https://drafts.csswg.org/cssom/#namespacedef-css
 */

// https://drafts.csswg.org/cssom/#the-css.escape()-method
[Exposed=(Window, Worker)]
namespace CSS {
  [Exposed=Window, Throws] CSSOMString escape(CSSOMString ident);
};

// https://drafts.csswg.org/css-conditional-3/#dom-css-supports
partial namespace CSS {
  [Exposed=Window] boolean supports(CSSOMString property, CSSOMString value);
  [Exposed=Window] boolean supports(CSSOMString conditionText);
};

// https://drafts.css-houdini.org/css-paint-api/#dom-css-paintworklet
partial namespace CSS {
    [Exposed=Window, SameObject, Pref="dom_worklet_enabled"] readonly attribute Worklet paintWorklet;
};

// https://drafts.css-houdini.org/css-properties-values-api/#the-registerproperty-function
dictionary PropertyDefinition {
  required DOMString name;
           DOMString syntax       = "*";
  required boolean   inherits;
           DOMString initialValue;
};

partial namespace CSS {
  [Exposed=Window, Throws] undefined registerProperty(PropertyDefinition definition);
};

// https://drafts.css-houdini.org/css-typed-om-1/#numeric-factory-functions
partial namespace CSS {
  CSSUnitValue number(double value);
  CSSUnitValue percent(double value);
  CSSUnitValue em(double value);
  CSSUnitValue ex(double value);
  CSSUnitValue ch(double value);
  CSSUnitValue ic(double value);
  CSSUnitValue rem(double value);
  CSSUnitValue lh(double value);
  CSSUnitValue rlh(double value);
  CSSUnitValue vw(double value);
  CSSUnitValue vh(double value);
  CSSUnitValue vi(double value);
  CSSUnitValue vb(double value);
  CSSUnitValue vmin(double value);
  CSSUnitValue vmax(double value);
  CSSUnitValue cm(double value);
  CSSUnitValue mm(double value);
  CSSUnitValue Q(double value);
  CSSUnitValue in(double value);
  CSSUnitValue pt(double value);
  CSSUnitValue pc(double value);
  CSSUnitValue px(double value);
  CSSUnitValue deg(double value);
  CSSUnitValue grad(double value);
  CSSUnitValue rad(double value);
  CSSUnitValue turn(double value);
  CSSUnitValue s(double value);
  CSSUnitValue ms(double value);
  CSSUnitValue Hz(double value);
  CSSUnitValue kHz(double value);
  CSSUnitValue dpi(double value);
  CSSUnitValue dpcm(double value);
  CSSUnitValue dppx(double value);
  CSSUnitValue fr(double value);
};
