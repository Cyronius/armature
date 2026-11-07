/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/types.html#InterfaceSVGStringList
[Exposed=Window]
interface SVGStringList {

  readonly attribute unsigned long length;
  readonly attribute unsigned long numberOfItems;

  undefined clear();
  DOMString initialize(DOMString newItem);
  [Throws] getter DOMString getItem(unsigned long index);
  DOMString insertItemBefore(DOMString newItem, unsigned long index);
  [Throws] DOMString replaceItem(DOMString newItem, unsigned long index);
  [Throws] DOMString removeItem(unsigned long index);
  DOMString appendItem(DOMString newItem);
  [Throws] setter undefined (unsigned long index, DOMString newItem);
};
