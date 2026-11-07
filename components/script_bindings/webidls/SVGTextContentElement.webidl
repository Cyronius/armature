/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/text.html#InterfaceSVGTextContentElement
[Exposed=Window, Abstract]
interface SVGTextContentElement : SVGGraphicsElement {
  const unsigned short LENGTHADJUST_UNKNOWN = 0;
  const unsigned short LENGTHADJUST_SPACING = 1;
  const unsigned short LENGTHADJUST_SPACINGANDGLYPHS = 2;

  [SameObject] readonly attribute SVGAnimatedLength textLength;
  [SameObject] readonly attribute SVGAnimatedEnumeration lengthAdjust;

  long getNumberOfChars();
  float getComputedTextLength();
  [Throws] float getSubStringLength(unsigned long charnum, unsigned long nchars);
  [Throws] DOMPoint getStartPositionOfChar(unsigned long charnum);
  [Throws] DOMPoint getEndPositionOfChar(unsigned long charnum);
  [Throws] DOMRect getExtentOfChar(unsigned long charnum);
  [Throws] float getRotationOfChar(unsigned long charnum);
  long getCharNumAtPosition(optional DOMPointInit point = {});
  [Throws] undefined selectSubString(unsigned long charnum, unsigned long nchars);
};
