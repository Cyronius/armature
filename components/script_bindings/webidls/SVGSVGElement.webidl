/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/struct.html#InterfaceSVGSVGElement
[Exposed=Window]
interface SVGSVGElement : SVGGraphicsElement {

  [SameObject] readonly attribute SVGAnimatedLength x;
  [SameObject] readonly attribute SVGAnimatedLength y;
  [SameObject] readonly attribute SVGAnimatedLength width;
  [SameObject] readonly attribute SVGAnimatedLength height;

  attribute float currentScale;
  [SameObject] readonly attribute DOMPoint currentTranslate;

  NodeList getIntersectionList(DOMRectReadOnly rect, SVGElement? referenceElement);
  NodeList getEnclosureList(DOMRectReadOnly rect, SVGElement? referenceElement);
  boolean checkIntersection(SVGElement element, DOMRectReadOnly rect);
  boolean checkEnclosure(SVGElement element, DOMRectReadOnly rect);

  undefined deselectAll();

  [NewObject] SVGNumber createSVGNumber();
  [NewObject] SVGLength createSVGLength();
  [NewObject] SVGAngle createSVGAngle();
  [NewObject] DOMPoint createSVGPoint();
  [NewObject] DOMMatrix createSVGMatrix();
  [NewObject] DOMRect createSVGRect();
  [NewObject] SVGTransform createSVGTransform();
  [NewObject, Throws] SVGTransform createSVGTransformFromMatrix(optional DOMMatrix2DInit matrix = {});

  Element? getElementById(DOMString elementId);

  // Deprecated methods that have no effect when called,
  // but which are kept for compatibility reasons.
  unsigned long suspendRedraw(unsigned long maxWaitMilliseconds);
  undefined unsuspendRedraw(unsigned long suspendHandleID);
  undefined unsuspendRedrawAll();
  undefined forceRedraw();
};

//SVGSVGElement includes SVGZoomAndPan;
//SVGSVGElement includes WindowEventHandlers;

SVGSVGElement includes SVGFitToViewBox;
