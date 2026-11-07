/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/shapes.html#InterfaceSVGPointList
[Exposed=Window]
interface SVGPointList {

  readonly attribute unsigned long length;
  readonly attribute unsigned long numberOfItems;

  [Throws] undefined clear();
  [Throws] DOMPoint initialize(DOMPoint newItem);
  [Throws] getter DOMPoint getItem(unsigned long index);
  [Throws] DOMPoint insertItemBefore(DOMPoint newItem, unsigned long index);
  [Throws] DOMPoint replaceItem(DOMPoint newItem, unsigned long index);
  [Throws] DOMPoint removeItem(unsigned long index);
  [Throws] DOMPoint appendItem(DOMPoint newItem);
  [Throws] setter undefined (unsigned long index, DOMPoint newItem);
};
