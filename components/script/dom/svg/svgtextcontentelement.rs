/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::context::JSContext;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::DOMPointBinding::DOMPointInit;
use crate::dom::bindings::codegen::Bindings::SVGTextContentElementBinding::SVGTextContentElementMethods;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::dompoint::DOMPoint;
use crate::dom::domrect::DOMRect;
use crate::dom::node::NodeTraits;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::svg::svggraphicselement::SVGGraphicsElement;
use crate::dom::svg::svgtext;
use crate::dom::svg::svgtypes::{SVGLengthValue, svg_animated_enumeration, svg_animated_length};

#[dom_struct]
pub(crate) struct SVGTextContentElement {
    svggraphicselement: SVGGraphicsElement,
}

impl SVGTextContentElement {
    pub(crate) fn new_inherited(
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGTextContentElement {
        SVGTextContentElement::new_inherited_with_state(
            ElementState::empty(),
            tag_name,
            prefix,
            document,
        )
    }

    pub(crate) fn new_inherited_with_state(
        state: ElementState,
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGTextContentElement {
        SVGTextContentElement {
            svggraphicselement: SVGGraphicsElement::new_inherited_with_state(
                state, tag_name, prefix, document,
            ),
        }
    }
}

impl VirtualMethods for SVGTextContentElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGraphicsElement>() as &dyn VirtualMethods)
    }
}

impl SVGTextContentElement {
    fn characters(&self, cx: &mut JSContext) -> Vec<svgtext::SVGTextCharacter> {
        svgtext::element_characters(cx, self.upcast())
    }

    /// The character at UTF-16 index `charnum`, or an `IndexSizeError`.
    fn character(&self, cx: &mut JSContext, charnum: u32) -> Fallible<svgtext::SVGTextCharacter> {
        let characters = self.characters(cx);
        let index = svgtext::character_at_utf16_index(&characters, charnum)
            .ok_or(Error::IndexSize(Some("There is no such character".into())))?;
        Ok(characters
            .into_iter()
            .nth(index)
            .expect("index is in range"))
    }
}

impl SVGTextContentElementMethods<crate::DomTypeHolder> for SVGTextContentElement {
    /// <https://svgwg.org/svg2-draft/text.html#__svg__SVGTextContentElement__getNumberOfChars>
    fn GetNumberOfChars(&self, cx: &mut JSContext) -> i32 {
        svgtext::utf16_length(&self.characters(cx)) as i32
    }

    /// <https://svgwg.org/svg2-draft/text.html#__svg__SVGTextContentElement__getComputedTextLength>
    fn GetComputedTextLength(&self, cx: &mut JSContext) -> Finite<f32> {
        let length: f64 = self
            .characters(cx)
            .iter()
            .map(|character| character.advance)
            .sum();
        Finite::wrap(length as f32)
    }

    /// <https://svgwg.org/svg2-draft/text.html#__svg__SVGTextContentElement__getSubStringLength>
    fn GetSubStringLength(
        &self,
        cx: &mut JSContext,
        charnum: u32,
        nchars: u32,
    ) -> Fallible<Finite<f32>> {
        let characters = self.characters(cx);
        let count = svgtext::utf16_length(&characters) as u32;
        if charnum >= count {
            return Err(Error::IndexSize(Some("There is no such character".into())));
        }
        let end = charnum.saturating_add(nchars).min(count);
        let mut offset = 0;
        let mut length = 0.0;
        for character in &characters {
            if offset >= charnum && offset < end {
                length += character.advance;
            }
            offset += character.utf16_length as u32;
        }
        Ok(Finite::wrap(length as f32))
    }

    /// <https://svgwg.org/svg2-draft/text.html#__svg__SVGTextContentElement__getStartPositionOfChar>
    fn GetStartPositionOfChar(
        &self,
        cx: &mut JSContext,
        charnum: u32,
    ) -> Fallible<DomRoot<DOMPoint>> {
        let character = self.character(cx, charnum)?;
        Ok(DOMPoint::new_svg(
            cx,
            &self.owner_global(),
            character.x,
            character.y,
        ))
    }

    /// <https://svgwg.org/svg2-draft/text.html#__svg__SVGTextContentElement__getEndPositionOfChar>
    fn GetEndPositionOfChar(
        &self,
        cx: &mut JSContext,
        charnum: u32,
    ) -> Fallible<DomRoot<DOMPoint>> {
        let character = self.character(cx, charnum)?;
        let angle = character.rotate.to_radians();
        Ok(DOMPoint::new_svg(
            cx,
            &self.owner_global(),
            character.x + character.advance * angle.cos(),
            character.y + character.advance * angle.sin(),
        ))
    }

    /// <https://svgwg.org/svg2-draft/text.html#__svg__SVGTextContentElement__getExtentOfChar>
    fn GetExtentOfChar(&self, cx: &mut JSContext, charnum: u32) -> Fallible<DomRoot<DOMRect>> {
        let extent = self.character(cx, charnum)?.extent();
        Ok(DOMRect::new_svg(
            cx,
            &self.owner_global(),
            extent.x0,
            extent.y0,
            extent.width(),
            extent.height(),
        ))
    }

    /// <https://svgwg.org/svg2-draft/text.html#__svg__SVGTextContentElement__getRotationOfChar>
    fn GetRotationOfChar(&self, cx: &mut JSContext, charnum: u32) -> Fallible<Finite<f32>> {
        let character = self.character(cx, charnum)?;
        Ok(Finite::wrap(
            svgtext::character_rotation(cx, &character) as f32
        ))
    }

    /// <https://svgwg.org/svg2-draft/text.html#__svg__SVGTextContentElement__getCharNumAtPosition>
    fn GetCharNumAtPosition(&self, cx: &mut JSContext, point: &DOMPointInit) -> i32 {
        let point = kurbo::Point::new(point.x, point.y);
        let mut offset = 0;
        for character in self.characters(cx) {
            if character.extent().contains(point) {
                return offset;
            }
            offset += character.utf16_length as i32;
        }
        -1
    }

    /// <https://svgwg.org/svg2-draft/text.html#__svg__SVGTextContentElement__selectSubString>
    fn SelectSubString(&self, cx: &mut JSContext, charnum: u32, _nchars: u32) -> ErrorResult {
        // Selecting SVG text is not supported; only the arguments are checked.
        if charnum as usize >= svgtext::utf16_length(&self.characters(cx)) {
            return Err(Error::IndexSize(Some("There is no such character".into())));
        }
        Ok(())
    }

    svg_animated_length!(
        TextLength,
        "textLength",
        Horizontal,
        SVGLengthValue::number(0.0)
    );
    svg_animated_enumeration!(
        LengthAdjust,
        "lengthAdjust",
        "lengthAdjust",
        &[("spacing", 1), ("spacingAndGlyphs", 2)],
        1
    );
}
