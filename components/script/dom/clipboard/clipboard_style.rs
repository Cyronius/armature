/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use cssparser::ToCss;
use html5ever::{local_name, ns};
use indexmap::IndexMap;
use js::context::JSContext;
use rustc_hash::FxHashMap;
use style::properties::{
    Importance, LonghandId, PropertyDeclaration, PropertyDeclarationBlock, PropertyDeclarationId,
    parse_style_attribute,
};
use style::shared_lock::SharedRwLock;
use style::stylesheets::{
    AllowImportRules, CssRule, CssRuleType, Origin, StylesheetContents, UrlExtraData,
};
use style::values::generics::svg::SVGPaintKind;

use crate::dom::bindings::codegen::Bindings::ElementBinding::ElementMethods;
use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::element::Element;
use crate::dom::node::{Node, NodeTraits};

type Declarations = IndexMap<LonghandId, (PropertyDeclaration, Importance, u32)>;

/// Preserve styles that can travel with a clipboard fragment without copying
/// selectors, resource loads, or declarations that depend on the destination.
pub(super) fn inline_clipboard_styles(cx: &mut JSContext, fragment: &Node) {
    let document = fragment.owner_document();
    let url_data = UrlExtraData::from(document.url().into_url());
    let quirks_mode = document.quirks_mode();
    let lock = SharedRwLock::new();
    let mut styles = FxHashMap::<DomRoot<Element>, Declarations>::default();

    if let Ok(elements) = fragment.query_selector_all(cx, "style".into()) {
        let elements: Vec<_> = elements
            .iter(cx.no_gc())
            .filter_map(|node| DomRoot::downcast::<Element>(node.as_rooted()))
            .collect();
        for style in elements {
            let Some(css) = style.upcast::<Node>().GetTextContent() else {
                style.Remove(cx);
                continue;
            };
            let sheet = StylesheetContents::from_str(
                &css.str(),
                url_data.clone(),
                Origin::Author,
                &lock,
                None,
                None,
                quirks_mode,
                AllowImportRules::No,
                None,
            );
            let guard = lock.read();
            for rule in &sheet.rules.read_with(&guard).0 {
                let CssRule::Style(rule) = rule else { continue };
                let rule = rule.read_with(&guard);
                let block = rule.block.read_with(&guard);
                for selector in rule.selectors.slice() {
                    let Ok(matches) =
                        fragment.query_selector_all(cx, selector.to_css_string().into())
                    else {
                        continue;
                    };
                    for matched in matches.iter(cx.no_gc()) {
                        let element = DomRoot::downcast::<Element>(matched.as_rooted()).unwrap();
                        cascade_declarations(
                            styles.entry(element).or_default(),
                            block,
                            selector.specificity(),
                        );
                    }
                }
            }
            style.Remove(cx);
        }
    }

    if let Ok(elements) = fragment.query_selector_all(cx, "[style]".into()) {
        let elements: Vec<_> = elements
            .iter(cx.no_gc())
            .filter_map(|node| DomRoot::downcast::<Element>(node.as_rooted()))
            .collect();
        for element in elements {
            let value = element.GetAttribute(cx, "style".into()).unwrap_or_default();
            let block = parse_style_attribute(
                &value.str(),
                &url_data,
                None,
                quirks_mode,
                CssRuleType::Style,
            );
            element.RemoveAttribute(cx, "style".into());
            cascade_declarations(styles.entry(element).or_default(), &block, u32::MAX);
        }
    }

    for (element, declarations) in styles {
        if declarations.is_empty() || !fragment.Contains(Some(element.upcast())) {
            continue;
        }
        let mut block = PropertyDeclarationBlock::new();
        for (declaration, importance, _) in declarations.into_values() {
            block.push(declaration, importance);
        }
        let mut serialized = String::new();
        block.to_css(&mut serialized).unwrap();
        let name = local_name!("style");
        let value = element.parse_attribute(&ns!(), &name, serialized.into());
        element.set_attribute(cx, &name, value);
    }
}

fn cascade_declarations(
    result: &mut Declarations,
    incoming: &PropertyDeclarationBlock,
    specificity: u32,
) {
    for (declaration, importance) in incoming.declaration_importance_iter() {
        let PropertyDeclarationId::Longhand(id) = declaration.id() else {
            continue;
        };
        if !is_clipboard_style(declaration, id) {
            continue;
        }
        if result.get(&id).is_some_and(|(_, previous, weight)| {
            (previous.important(), *weight) > (importance.important(), specificity)
        }) {
            continue;
        }
        result.insert(id, (declaration.clone(), importance, specificity));
    }
}

fn is_clipboard_style(declaration: &PropertyDeclaration, id: LonghandId) -> bool {
    if matches!(declaration, PropertyDeclaration::WithVariables(..)) {
        return false;
    }
    match declaration {
        PropertyDeclaration::Fill(paint) | PropertyDeclaration::Stroke(paint) => {
            return !matches!(paint.kind, SVGPaintKind::PaintServer(_));
        },
        _ => (),
    }
    // Enumerate properties whose value grammars cannot reference resources.
    matches!(
        id,
        LonghandId::BackgroundColor |
            LonghandId::Color |
            LonghandId::FillOpacity |
            LonghandId::FontFamily |
            LonghandId::FontSize |
            LonghandId::FontStyle |
            LonghandId::FontWeight |
            LonghandId::LetterSpacing |
            LonghandId::LineHeight |
            LonghandId::Opacity |
            LonghandId::StrokeOpacity |
            LonghandId::StrokeWidth |
            LonghandId::TextAlign |
            LonghandId::TextDecorationColor |
            LonghandId::TextDecorationLine |
            LonghandId::TextDecorationStyle |
            LonghandId::TextIndent |
            LonghandId::TextTransform |
            LonghandId::WhiteSpaceCollapse |
            LonghandId::WordSpacing
    )
}
