/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://svgwg.org/svg2-draft/types.html#InterfaceSVGNumberList
[Exposed=Window]
interface SVGNumberList {

  readonly attribute unsigned long length;
  readonly attribute unsigned long numberOfItems;

  [Throws] undefined clear();
  [Throws] SVGNumber initialize(SVGNumber newItem);
  [Throws] getter SVGNumber getItem(unsigned long index);
  [Throws] SVGNumber insertItemBefore(SVGNumber newItem, unsigned long index);
  [Throws] SVGNumber replaceItem(SVGNumber newItem, unsigned long index);
  [Throws] SVGNumber removeItem(unsigned long index);
  [Throws] SVGNumber appendItem(SVGNumber newItem);
  [Throws] setter undefined (unsigned long index, SVGNumber newItem);
};
