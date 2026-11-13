/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Resolve CSS counters in tree order before box construction, which can run in parallel.

use std::collections::HashMap;

use layout_api::{LayoutNode, PseudoElementChain};
use script::layout_dom::ServoLayoutNode;
use style::dom::{NodeInfo, OpaqueNode};
use style::properties::longhands::contain::computed_value::T as Contain;
use style::selector_parser::PseudoElement;
use style::values::computed::ContentItem;
use style::values::generics::counters::Content;
use style::values::specified::Quotes;
use stylo_atoms::Atom;

use crate::context::LayoutContext;
use crate::quotes::quotes_for_lang;
use crate::style_ext::Display;

type CounterKey = (OpaqueNode, PseudoElementChain);

#[derive(Clone, Default)]
pub(crate) struct CounterSnapshot {
    counters: Vec<(Atom, i32)>,
    quote_text: Vec<Option<String>>,
}

impl CounterSnapshot {
    pub(crate) fn value(&self, name: &Atom) -> i32 {
        self.counters
            .iter()
            .rev()
            .find(|(counter_name, _)| counter_name == name)
            .map_or(0, |(_, value)| *value)
    }

    pub(crate) fn values(&self, name: &Atom) -> Vec<i32> {
        let values: Vec<_> = self
            .counters
            .iter()
            .filter_map(|(counter_name, value)| (counter_name == name).then_some(*value))
            .collect();
        if values.is_empty() { vec![0] } else { values }
    }

    pub(crate) fn quote_text(&self, index: usize) -> Option<&str> {
        self.quote_text.get(index).and_then(|text| text.as_deref())
    }
}

#[derive(Default)]
pub(crate) struct CounterSnapshots(HashMap<CounterKey, CounterSnapshot>);

impl CounterSnapshots {
    pub(crate) fn get(&self, node: ServoLayoutNode<'_>) -> Option<&CounterSnapshot> {
        self.0.get(&(node.opaque(), node.pseudo_element_chain()))
    }
}

struct CounterInstance {
    name: Atom,
    value: i32,
    reversed: bool,
    creator_depth: usize,
}

#[derive(Default)]
struct CounterBuilder {
    instances: Vec<CounterInstance>,
    snapshots: CounterSnapshots,
    quote_depth: usize,
}

impl CounterBuilder {
    fn ensure(&mut self, scope: &mut Vec<usize>, name: &Atom, depth: usize) -> usize {
        if let Some(index) = scope
            .iter()
            .rev()
            .find(|&&index| &self.instances[index].name == name)
        {
            return *index;
        }
        self.instantiate(scope, name.clone(), 0, false, depth)
    }

    fn instantiate(
        &mut self,
        scope: &mut Vec<usize>,
        name: Atom,
        value: i32,
        reversed: bool,
        depth: usize,
    ) -> usize {
        if let Some(position) = scope
            .iter()
            .rposition(|&index| self.instances[index].name == name) &&
            self.instances[scope[position]].creator_depth >= depth
        {
            scope.remove(position);
        }
        let index = self.instances.len();
        self.instances.push(CounterInstance {
            name,
            value,
            reversed,
            creator_depth: depth,
        });
        scope.push(index);
        index
    }

    fn record(
        &mut self,
        node: ServoLayoutNode<'_>,
        scope: &[usize],
        quote_text: Vec<Option<String>>,
    ) {
        self.snapshots.0.insert(
            (node.opaque(), node.pseudo_element_chain()),
            CounterSnapshot {
                counters: scope
                    .iter()
                    .map(|&index| {
                        let instance = &self.instances[index];
                        (instance.name.clone(), instance.value)
                    })
                    .collect(),
                quote_text,
            },
        );
    }

    fn quote_text(
        &mut self,
        item: &ContentItem,
        style: &style::properties::ComputedValues,
    ) -> Option<String> {
        // https://drafts.csswg.org/css-content-3/#quotes
        let depth = match item {
            ContentItem::OpenQuote | ContentItem::NoOpenQuote => {
                let depth = self.quote_depth;
                self.quote_depth = self.quote_depth.saturating_add(1);
                depth
            },
            ContentItem::CloseQuote | ContentItem::NoCloseQuote => {
                if self.quote_depth == 0 {
                    return Some(String::new());
                }
                self.quote_depth -= 1;
                self.quote_depth
            },
            _ => return None,
        };
        if matches!(item, ContentItem::NoOpenQuote | ContentItem::NoCloseQuote) {
            return None;
        }
        match &style.get_list().quotes {
            Quotes::QuoteList(list) => list.0.get(depth).or_else(|| list.0.last()).map(|pair| {
                if matches!(item, ContentItem::OpenQuote) {
                    pair.opening.to_string()
                } else {
                    pair.closing.to_string()
                }
            }),
            Quotes::Auto => {
                let lang = &style.get_font()._x_lang;
                let pair = quotes_for_lang(lang.0.as_ref(), depth);
                Some(if matches!(item, ContentItem::OpenQuote) {
                    pair.opening.to_string()
                } else {
                    pair.closing.to_string()
                })
            },
        }
    }

    fn initial_reversed_value(
        node: ServoLayoutNode<'_>,
        name: &Atom,
        context: &LayoutContext,
    ) -> i32 {
        // A reversed counter without an explicit start counts back from its first
        // counter-set, or from the increments in its scope if no set exists.
        fn scan(
            node: ServoLayoutNode<'_>,
            name: &Atom,
            context: &LayoutContext,
            total: &mut i32,
            last_nonzero_increment_negated: &mut i32,
        ) -> bool {
            for child in node
                .flat_tree_children()
                .filter(ServoLayoutNode::is_element)
            {
                let style = child.style(&context.style_context);
                if matches!(Display::from(style.get_box().display), Display::None) {
                    continue;
                }
                let counters = style.get_counters();
                if counters
                    .counter_reset
                    .iter()
                    .any(|pair| &pair.name.0 == name)
                {
                    continue;
                }
                let explicit_increments: Vec<_> = counters
                    .counter_increment
                    .iter()
                    .filter(|pair| &pair.name.0 == name)
                    .collect();
                let increment = if explicit_increments.is_empty() &&
                    name == &Atom::from("list-item") &&
                    style.get_box().display.is_list_item()
                {
                    -1
                } else {
                    explicit_increments
                        .iter()
                        .fold(0i32, |sum, pair| sum.saturating_add(pair.value))
                };
                let increment_negated = increment.saturating_neg();
                if increment_negated != 0 {
                    *last_nonzero_increment_negated = increment_negated;
                }
                if let Some(pair) = counters
                    .counter_set
                    .iter()
                    .rev()
                    .find(|pair| &pair.name.0 == name)
                {
                    *total = total.saturating_add(pair.value);
                    return true;
                }
                *total = total.saturating_add(increment_negated);
                if !style.get_box().contain.contains(Contain::STYLE) &&
                    scan(child, name, context, total, last_nonzero_increment_negated)
                {
                    return true;
                }
            }
            false
        }
        let mut total = 0i32;
        let mut last_nonzero_increment_negated = 0i32;
        scan(
            node,
            name,
            context,
            &mut total,
            &mut last_nonzero_increment_negated,
        );
        total.saturating_add(last_nonzero_increment_negated)
    }

    fn apply_properties(
        &mut self,
        node: ServoLayoutNode<'_>,
        scope: &mut Vec<usize>,
        depth: usize,
        context: &LayoutContext,
    ) {
        let style = node.style(&context.style_context);
        let counters = style.get_counters();

        for pair in counters.counter_reset.iter() {
            let value = if pair.is_reversed && pair.value == i32::MIN {
                Self::initial_reversed_value(node, &pair.name.0, context)
            } else {
                pair.value
            };
            self.instantiate(scope, pair.name.0.clone(), value, pair.is_reversed, depth);
        }

        for pair in counters.counter_increment.iter() {
            let index = self.ensure(scope, &pair.name.0, depth);
            self.instances[index].value = self.instances[index].value.saturating_add(pair.value);
        }
        let list_item = Atom::from("list-item");
        if style.get_box().display.is_list_item() &&
            !counters
                .counter_increment
                .iter()
                .any(|pair| pair.name.0 == list_item)
        {
            let index = self.ensure(scope, &list_item, depth);
            let delta = if self.instances[index].reversed {
                -1
            } else {
                1
            };
            self.instances[index].value = self.instances[index].value.saturating_add(delta);
        }

        for pair in counters.counter_set.iter() {
            let index = self.ensure(scope, &pair.name.0, depth);
            self.instances[index].value = pair.value;
        }

        let quote_text = if let Content::Items(items) = &counters.content {
            let mut quote_text = Vec::with_capacity(items.items.len());
            for item in &items.items {
                match item {
                    ContentItem::Counter(name, _) | ContentItem::Counters(name, _, _) => {
                        self.ensure(scope, &name.0, depth);
                    },
                    _ => {},
                }
                quote_text.push(self.quote_text(item, &style));
            }
            quote_text
        } else {
            Vec::new()
        };
        self.record(node, scope, quote_text);
    }

    fn process_pseudo(
        &mut self,
        node: ServoLayoutNode<'_>,
        pseudo: PseudoElement,
        mut scope: Vec<usize>,
        depth: usize,
        context: &LayoutContext,
    ) -> Option<Vec<usize>> {
        let pseudo_node = node.with_pseudo(pseudo)?;
        let style = pseudo_node.style(&context.style_context);
        if matches!(Display::from(style.get_box().display), Display::None) ||
            (pseudo.is_before_or_after() && style.ineffective_content_property())
        {
            return None;
        }
        self.apply_properties(pseudo_node, &mut scope, depth, context);
        Some(scope)
    }

    fn process_element(
        &mut self,
        node: ServoLayoutNode<'_>,
        mut scope: Vec<usize>,
        depth: usize,
        context: &LayoutContext,
    ) -> Vec<usize> {
        let style = node.style(&context.style_context);
        if matches!(Display::from(style.get_box().display), Display::None) {
            return scope;
        }
        let inherited_quote_depth = self.quote_depth;
        self.apply_properties(node, &mut scope, depth, context);

        // Style containment makes the element a counter-scope boundary. Counters
        // created on the element remain visible to its descendants, while
        // inherited counters and descendant updates stay outside that scope.
        let descendant_scope = if style.get_box().contain.contains(Contain::STYLE) {
            scope
                .iter()
                .copied()
                .filter(|&index| self.instances[index].creator_depth >= depth)
                .collect()
        } else {
            scope.clone()
        };
        let mut child_scope = None;
        if style.get_box().display.is_list_item() {
            child_scope = self.process_pseudo(
                node,
                PseudoElement::Marker,
                descendant_scope.clone(),
                depth + 1,
                context,
            );
        }
        if let Some(before_scope) = self.process_pseudo(
            node,
            PseudoElement::Before,
            child_scope
                .clone()
                .unwrap_or_else(|| descendant_scope.clone()),
            depth + 1,
            context,
        ) {
            child_scope = Some(before_scope);
        }

        for child in node
            .flat_tree_children()
            .filter(ServoLayoutNode::is_element)
        {
            child_scope = Some(self.process_element(
                child,
                child_scope.unwrap_or_else(|| descendant_scope.clone()),
                depth + 1,
                context,
            ));
        }
        let last_child_scope = child_scope.unwrap_or(descendant_scope);
        let after_scope = self.process_pseudo(
            node,
            PseudoElement::After,
            last_child_scope.clone(),
            depth + 1,
            context,
        );
        if style.get_box().contain.contains(Contain::STYLE) {
            self.quote_depth = inherited_quote_depth;
        }
        if matches!(Display::from(style.get_box().display), Display::Contents) &&
            !style.get_box().contain.contains(Contain::STYLE)
        {
            after_scope.unwrap_or(last_child_scope)
        } else {
            scope
        }
    }
}

pub(crate) fn build_counter_snapshots(
    root: ServoLayoutNode<'_>,
    context: &LayoutContext,
) -> CounterSnapshots {
    let mut builder = CounterBuilder::default();
    builder.process_element(root, Vec::new(), 0, context);
    builder.snapshots
}
