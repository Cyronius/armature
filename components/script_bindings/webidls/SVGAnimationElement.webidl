/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://w3c.github.io/svgwg/specs/animations/#InterfaceSVGAnimationElement
[Exposed=Window]
interface SVGAnimationElement : SVGElement {
  readonly attribute SVGElement? targetElement;

  [Throws] float getStartTime();
  float getCurrentTime();
  [Throws] float getSimpleDuration();

  undefined beginElement();
  undefined beginElementAt(float offset);
  undefined endElement();
  undefined endElementAt(float offset);
};

SVGAnimationElement includes SVGTests;
