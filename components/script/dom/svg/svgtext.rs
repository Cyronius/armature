/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The positions of the characters of SVG text, for the methods of
//! `SVGTextContentElement` and for `getBBox()`.
//!
//! Servo renders SVG through resvg, so layout does not lay out SVG text. This lays out
//! each `text` element with a simple model: characters are collapsed as with
//! `white-space: normal` (or preserved with `xml:space="preserve"`), positioned by the
//! `x`, `y`, `dx`, `dy` and `rotate` attributes, advanced by the horizontal advance of
//! their glyph, and text chunks are aligned by `text-anchor`. There is no shaping, kerning,
//! bidi reordering, `textPath` layout or vertical text.
//!
//! <https://svgwg.org/svg2-draft/text.html#TextLayoutAlgorithm>

use html5ever::{LocalName, local_name, ns};
use icu_locale_core::subtags::Language;
use js::context::JSContext;
use kurbo::Rect;
use unicode_vo::{Orientation, char_orientation};

use crate::dom::bindings::codegen::Bindings::CharacterDataBinding::CharacterDataMethods;
use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::characterdata::CharacterData;
use crate::dom::element::Element;
use crate::dom::iterators::ShadowIncluding;
use crate::dom::node::{Node, NodeTraits};
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svggeometry;
use crate::dom::svg::svglength::resolve_value;
use crate::dom::svg::svgtypes::{SVGLengthDirection, SVGLengthValue, parse_number, split_list};
use crate::dom::text::Text;

/// A laid out character of SVG text.
pub(crate) struct SVGTextCharacter {
    pub(crate) character: char,
    /// The source text node and its UTF-16 offset before whitespace collapsing.
    pub(crate) source: DomRoot<Text>,
    pub(crate) source_offset: u32,
    /// The text content elements that contain the character, from the innermost one out to
    /// the `text` element.
    pub(crate) elements: Vec<DomRoot<SVGElement>>,
    /// The number of UTF-16 code units of the character.
    pub(crate) utf16_length: usize,
    /// The origin of the glyph, on the baseline.
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) advance: f64,
    pub(crate) ascent: f64,
    pub(crate) descent: f64,
    /// The rotation of the glyph, in degrees.
    pub(crate) rotate: f64,
}

/// The rotation exposed by `getRotationOfChar`, including the glyph orientation and
/// the tangent when the character is on a text path.
pub(crate) fn character_rotation(cx: &mut JSContext, glyph: &SVGTextCharacter) -> f64 {
    let writing_mode = svggeometry::computed_value(&glyph.elements[0], "writing-mode");
    let orientation = svggeometry::computed_value(&glyph.elements[0], "text-orientation");
    let upright = matches!(
        char_orientation(glyph.character),
        Orientation::Upright | Orientation::TransformedOrUpright
    );

    let path_angle = glyph
        .elements
        .iter()
        .find(|element| element.upcast::<Element>().local_name() == &local_name!("textPath"))
        .and_then(|text_path| {
            let geometry = if let Some(data) = text_path.svg_attribute(&local_name!("path")) {
                svggeometry::SVGShapeGeometry::Path(svggeometry::parse_path_data(&data))
            } else {
                let element = text_path.upcast::<Element>();
                let href = element
                    .get_attribute_string_value_with_namespace(&ns!(), &local_name!("href"))
                    .or_else(|| {
                        element.get_attribute_string_value_with_namespace(
                            &ns!(xlink),
                            &local_name!("href"),
                        )
                    })?;
                let id = href.trim().strip_prefix('#')?;
                let document = text_path.upcast::<Node>().owner_doc();
                let referenced =
                    document.get_element_by_id(cx.no_gc(), &stylo_atoms::Atom::from(id))?;
                let path = DomRoot::downcast::<SVGElement>(referenced)?;
                svggeometry::shape_geometry(cx, &path)?
            };
            let center = glyph.x + glyph.advance / 2.0;
            let start = geometry.point_at_length((center - 0.01).max(0.0))?;
            let end = geometry.point_at_length(center + 0.01)?;
            Some((end.y - start.y).atan2(end.x - start.x).to_degrees())
        });

    let orientation_angle = match writing_mode.as_str() {
        "sideways-rl" => {
            if path_angle.is_some() {
                0.0
            } else {
                90.0
            }
        },
        "sideways-lr" => {
            if path_angle.is_some() {
                180.0
            } else {
                -90.0
            }
        },
        "vertical-rl" | "vertical-lr" => {
            if path_angle.is_some() {
                if orientation == "mixed" && upright {
                    -90.0
                } else {
                    0.0
                }
            } else if orientation == "sideways" || (orientation == "mixed" && !upright) {
                90.0
            } else {
                0.0
            }
        },
        _ => 0.0,
    };
    glyph.rotate + path_angle.unwrap_or(0.0) + orientation_angle
}

impl SVGTextCharacter {
    /// The box of the glyph, its advance by the ascent and descent of its font.
    pub(crate) fn extent(&self) -> Rect {
        Rect::new(
            self.x,
            self.y - self.ascent,
            self.x + self.advance,
            self.y + self.descent,
        )
    }

    pub(crate) fn belongs_to(&self, element: &SVGElement) -> bool {
        self.elements.iter().any(|ancestor| &**ancestor == element)
    }
}

/// Whether `element` is a text content element whose text is laid out.
fn is_text_content_element(element: &Element) -> bool {
    *element.namespace() == ns!(svg) &&
        matches!(
            element.local_name(),
            &local_name!("text") |
                &local_name!("tspan") |
                &local_name!("textPath") |
                &local_name!("a")
        )
}

/// The `text` element that `element` belongs to.
pub(crate) fn text_root(element: &SVGElement) -> Option<DomRoot<SVGElement>> {
    element
        .upcast::<Node>()
        .inclusive_ancestors(ShadowIncluding::No)
        .filter_map(DomRoot::downcast::<SVGElement>)
        .find(|ancestor| ancestor.upcast::<Element>().local_name() == &local_name!("text"))
}

/// The characters of the text, with the text content elements that contain them.
fn collect_characters(
    root: &SVGElement,
) -> Vec<(char, DomRoot<Text>, u32, Vec<DomRoot<SVGElement>>)> {
    let mut characters = Vec::new();
    let preserve = |element: &SVGElement| {
        element
            .upcast::<Node>()
            .inclusive_ancestors(ShadowIncluding::No)
            .filter_map(DomRoot::downcast::<Element>)
            .find_map(|ancestor| {
                ancestor
                    .get_attribute_string_value_with_namespace(&ns!(xml), &LocalName::from("space"))
            })
            .is_some_and(|value| value == "preserve")
    };
    for node in root.upcast::<Node>().traverse_preorder(ShadowIncluding::No) {
        let Some(text) = node.downcast::<Text>() else {
            continue;
        };
        let Some(parent) = node.GetParentElement() else {
            continue;
        };
        // Only text whose ancestors up to the root are text content elements is laid out.
        let elements: Vec<DomRoot<SVGElement>> = parent
            .upcast::<Node>()
            .inclusive_ancestors(ShadowIncluding::No)
            .take_while(|ancestor| &**ancestor != root.upcast::<Node>())
            .chain(std::iter::once(DomRoot::from_ref(root.upcast::<Node>())))
            .filter_map(DomRoot::downcast::<SVGElement>)
            .collect();
        if elements
            .iter()
            .any(|element| !is_text_content_element(element.upcast()))
        {
            continue;
        }
        let preserve_space = preserve(&elements[0]);
        let data = String::from(text.upcast::<CharacterData>().Data());
        let mut source_offset = 0;
        for character in data.chars() {
            let offset = source_offset;
            source_offset += character.len_utf16() as u32;
            let character = match character {
                '\n' | '\r' if !preserve_space => continue,
                '\n' | '\r' | '\t' => ' ',
                other => other,
            };
            if !preserve_space && character == ' ' {
                // Collapse spaces, and drop leading ones.
                if characters
                    .last()
                    .is_none_or(|(previous, _, _, _): &(char, _, _, _)| *previous == ' ')
                {
                    continue;
                }
            }
            characters.push((character, DomRoot::from_ref(text), offset, elements.clone()));
        }
    }
    if characters
        .last()
        .is_some_and(|(character, _, _, elements)| *character == ' ' && !preserve(&elements[0]))
    {
        characters.pop();
    }
    characters
}

/// The lengths of the attribute `name` of `element` in user units.
fn length_list(
    cx: &mut JSContext,
    element: &SVGElement,
    name: &LocalName,
    direction: SVGLengthDirection,
) -> Vec<f64> {
    let Some(value) = element.svg_attribute(name) else {
        return Vec::new();
    };
    let Some(items) = split_list(&value) else {
        return Vec::new();
    };
    let mut lengths = Vec::new();
    for item in items {
        let Some(value) = SVGLengthValue::parse(item) else {
            return Vec::new();
        };
        lengths.push(resolve_value(cx, element, value, direction) as f64);
    }
    lengths
}

/// The positioning attributes of a text content element.
struct PositioningLists {
    x: Vec<f64>,
    y: Vec<f64>,
    dx: Vec<f64>,
    dy: Vec<f64>,
    rotate: Vec<f64>,
}

fn positioning_lists(cx: &mut JSContext, element: &SVGElement) -> PositioningLists {
    use SVGLengthDirection::{Horizontal, Vertical};
    let rotate = element
        .svg_attribute(&local_name!("rotate"))
        .and_then(|value| {
            split_list(&value)?
                .into_iter()
                .map(|item| parse_number(item).map(|number| number as f64))
                .collect::<Option<Vec<f64>>>()
        })
        .unwrap_or_default();
    PositioningLists {
        x: length_list(cx, element, &local_name!("x"), Horizontal),
        y: length_list(cx, element, &local_name!("y"), Vertical),
        dx: length_list(cx, element, &local_name!("dx"), Horizontal),
        dy: length_list(cx, element, &local_name!("dy"), Vertical),
        rotate,
    }
}

/// The advance, ascent and descent of `character` in the font of `element`.
fn glyph_metrics(element: &SVGElement, character: char, next: Option<char>) -> (f64, f64, f64) {
    let Some(style) = element.inherited_style() else {
        return (8.0, 12.8, 3.2);
    };
    let font = style.clone_font();
    // SVG text geometry is in user units, while the computed font size includes CSS zoom.
    let font_size = style::values::computed::Length::new(
        style
            .effective_zoom
            .unzoom(font.font_size.computed_size().px()),
    );
    let window = element.owner_window();
    let font_context = window.font_context();
    let font_group = font_context.font_group_with_size(font, font_size.into());
    let font_size = font_size.px() as f64;
    let Some(font) =
        font_group.find_by_codepoint(&font_context, character, next, Language::UNKNOWN)
    else {
        return (font_size / 2.0, font_size * 0.8, font_size * 0.2);
    };
    let advance = font
        .glyph_index(character)
        .map_or(font_size / 2.0, |glyph| font.glyph_h_advance(glyph));
    let metrics = font.metrics();
    (
        advance,
        metrics.ascent.to_f64_px(),
        metrics.descent.to_f64_px(),
    )
}

/// The value of `text-anchor` of `element`: 0 for start, 1 for middle and 2 for end.
fn text_anchor(element: &SVGElement) -> u8 {
    use style::properties::longhands::text_anchor::computed_value::T as TextAnchor;
    element
        .inherited_style()
        .map_or(0, |style| match style.get_inherited_svg().text_anchor {
            TextAnchor::Start => 0,
            TextAnchor::Middle => 1,
            TextAnchor::End => 2,
        })
}

/// Lay out the characters of the `text` element `root`.
pub(crate) fn layout_text(cx: &mut JSContext, root: &SVGElement) -> Vec<SVGTextCharacter> {
    let characters = collect_characters(root);

    // The positioning attributes of every text content element, and the index of the next
    // character of each.
    let mut lists: Vec<(DomRoot<SVGElement>, PositioningLists, usize)> = Vec::new();
    let mut laid_out: Vec<SVGTextCharacter> = Vec::new();
    let mut chunk_starts = Vec::new();
    let (mut pen_x, mut pen_y) = (0.0, 0.0);
    for (index, (character, source, source_offset, elements)) in characters.iter().enumerate() {
        let mut x = None;
        let mut y = None;
        let mut dx = None;
        let mut dy = None;
        let mut rotate = None;
        // The innermost element whose lists have a value for the character wins.
        for element in elements {
            let entry = match lists.iter().position(|(other, _, _)| other == element) {
                Some(position) => position,
                None => {
                    let element_lists = positioning_lists(cx, element);
                    lists.push((element.clone(), element_lists, 0));
                    lists.len() - 1
                },
            };
            let (_, element_lists, next) = &mut lists[entry];
            let position = *next;
            *next += character.len_utf16();
            x = x.or_else(|| element_lists.x.get(position).copied());
            y = y.or_else(|| element_lists.y.get(position).copied());
            dx = dx.or_else(|| element_lists.dx.get(position).copied());
            dy = dy.or_else(|| element_lists.dy.get(position).copied());
            rotate = rotate.or_else(|| {
                element_lists
                    .rotate
                    .get(position)
                    .or(element_lists.rotate.last())
                    .copied()
            });
        }
        if x.is_some() || y.is_some() || index == 0 {
            chunk_starts.push(index);
        }
        if let Some(x) = x {
            pen_x = x;
        }
        if let Some(y) = y {
            pen_y = y;
        }
        pen_x += dx.unwrap_or(0.0);
        pen_y += dy.unwrap_or(0.0);
        let next = characters.get(index + 1).map(|(next, _, _, _)| *next);
        let (advance, ascent, descent) = glyph_metrics(&elements[0], *character, next);
        laid_out.push(SVGTextCharacter {
            character: *character,
            source: source.clone(),
            source_offset: *source_offset,
            elements: elements.clone(),
            utf16_length: character.len_utf16(),
            x: pen_x,
            y: pen_y,
            advance,
            ascent,
            descent,
            rotate: rotate.unwrap_or(0.0),
        });
        pen_x += advance;
    }

    // Align each text chunk by its text-anchor.
    chunk_starts.push(laid_out.len());
    for chunk in chunk_starts.windows(2) {
        let (start, end) = (chunk[0], chunk[1]);
        if start >= end {
            continue;
        }
        let anchor = text_anchor(&laid_out[start].elements[0]);
        if anchor == 0 {
            continue;
        }
        let left = laid_out[start..end]
            .iter()
            .map(|character| character.x)
            .fold(f64::INFINITY, f64::min);
        let right = laid_out[start..end]
            .iter()
            .map(|character| character.x + character.advance)
            .fold(f64::NEG_INFINITY, f64::max);
        let shift = (right - left) * anchor as f64 / 2.0;
        for character in &mut laid_out[start..end] {
            character.x -= shift;
        }
    }
    laid_out
}

/// The laid out characters of the text content element `element`.
pub(crate) fn element_characters(
    cx: &mut JSContext,
    element: &SVGElement,
) -> Vec<SVGTextCharacter> {
    let Some(root) = text_root(element) else {
        return Vec::new();
    };
    layout_text(cx, &root)
        .into_iter()
        .filter(|character| character.belongs_to(element))
        .collect()
}

/// The index of the character that contains UTF-16 code unit `index`, if any.
pub(crate) fn character_at_utf16_index(
    characters: &[SVGTextCharacter],
    index: u32,
) -> Option<usize> {
    let mut offset = 0;
    for (position, character) in characters.iter().enumerate() {
        if (index as usize) < offset + character.utf16_length {
            return Some(position);
        }
        offset += character.utf16_length;
    }
    None
}

/// The number of UTF-16 code units of `characters`.
pub(crate) fn utf16_length(characters: &[SVGTextCharacter]) -> usize {
    characters
        .iter()
        .map(|character| character.utf16_length)
        .sum()
}
