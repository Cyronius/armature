/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/types.html#InterfaceSVGAngle
[Exposed=Window]
interface SVGAngle {

  // Angle Unit Types
  const unsigned short SVG_ANGLETYPE_UNKNOWN = 0;
  const unsigned short SVG_ANGLETYPE_UNSPECIFIED = 1;
  const unsigned short SVG_ANGLETYPE_DEG = 2;
  const unsigned short SVG_ANGLETYPE_RAD = 3;
  const unsigned short SVG_ANGLETYPE_GRAD = 4;

  readonly attribute unsigned short unitType;
  [SetterThrows]
           attribute float value;
  [SetterThrows]
           attribute float valueInSpecifiedUnits;
  [SetterThrows]
           attribute DOMString valueAsString;

  [Throws] undefined newValueSpecifiedUnits(unsigned short unitType, float valueInSpecifiedUnits);
  [Throws] undefined convertToSpecifiedUnits(unsigned short unitType);
};
