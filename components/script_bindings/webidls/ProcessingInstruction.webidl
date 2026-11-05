/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
/*
 * The origin of this IDL file is
 * https://dom.spec.whatwg.org/#interface-processinginstruction
 */

[Exposed=Window]
interface ProcessingInstruction : CharacterData {
  [Throws] constructor(DOMString target, optional DOMString data = "");

  [Constant]
  readonly attribute DOMString target;

  // https://github.com/WICG/declarative-partial-updates (tentative)
  [Pref="dom_processing_instruction_attributes_enabled"]
  boolean hasAttributes();
  [Pref="dom_processing_instruction_attributes_enabled"]
  sequence<DOMString> getAttributeNames();
  [Pref="dom_processing_instruction_attributes_enabled"]
  DOMString? getAttribute(DOMString name);
  [Pref="dom_processing_instruction_attributes_enabled", Throws]
  undefined setAttribute(DOMString name, DOMString value);
  [Pref="dom_processing_instruction_attributes_enabled"]
  undefined removeAttribute(DOMString name);
  [Pref="dom_processing_instruction_attributes_enabled", Throws]
  boolean toggleAttribute(DOMString name, optional boolean force);
  [Pref="dom_processing_instruction_attributes_enabled"]
  boolean hasAttribute(DOMString name);
};

// https://drafts.csswg.org/cssom/#requirements-on-user-agents-implementing-the-xml-stylesheet-processing-instruction
ProcessingInstruction includes LinkStyle;
