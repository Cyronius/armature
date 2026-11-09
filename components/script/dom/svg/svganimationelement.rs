/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::time::Duration;

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name, ns};
use script_bindings::cell::DomRefCell;
use script_bindings::num::Finite;
use servo_arc::Arc as ServoArc;
use style::color::ColorSpace;
use style::properties::longhands::animation_composition::computed_value::single_value::T as AnimationComposition;
use style::properties::{PropertyDeclarationBlock, parse_style_attribute};
use style::servo::animation::{AnimationSetKey, ScriptAnimation, ScriptKeyframe};
use style::stylesheets::{CssRuleType, UrlExtraData};
use style::values::animated::{Animate, Procedure, ToAnimatedZero};
use style::values::computed::TimingFunction;
use style::values::computed::color::Color as ComputedColor;
use style::values::generics::easing::{BeforeFlag, StepPosition, TimingKeyword};
use style::values::specified::SVGPathData;
use style::values::specified::color::Color;
use style_traits::{ParsingMode, ToCss};
use stylo_atoms::Atom;
use stylo_dom::ElementState;

use crate::css::css::{ANONYMOUS_CONTENT_URL_DATA, parser_context_for_anonymous_content};
use crate::dom::bindings::codegen::Bindings::DocumentBinding::DocumentMethods;
use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::codegen::Bindings::SVGAnimationElementBinding::SVGAnimationElementMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::element::Element;
use crate::dom::eventtarget::EventTarget;
use crate::dom::iterators::ShadowIncluding;
use crate::dom::navigator::navigatorinfo;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::node::{BindContext, Node, NodeDamage, NodeFlags, NodeTraits, UnbindContext};
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgstringlist::SVGStringList;

#[derive(Clone, Copy)]
pub(crate) enum SVGTimingBoundaryKind {
    End,
    Begin,
    Repeat(u32),
}

fn parse_smil_color(value: &str) -> Option<[f32; 4]> {
    let mut parser = cssparser::Parser::new(value);
    let context = parser_context_for_anonymous_content(
        CssRuleType::Style,
        ParsingMode::DEFAULT,
        &ANONYMOUS_CONTENT_URL_DATA,
    );
    let ComputedColor::Absolute(color) =
        Color::parse_and_compute(&context, &mut parser, None).ok()?
    else {
        return None;
    };
    parser.expect_exhausted().ok()?;
    let color = color.to_color_space(ColorSpace::Srgb);
    Some([
        color.components.0,
        color.components.1,
        color.components.2,
        color.alpha,
    ])
}

/// SMIL adds color channels in sRGB, then interpolates without premultiplying alpha.
fn sampled_smil_color_by(
    animation: &SVGAnimationElement,
    target: &SVGElement,
    property: &str,
    progress: f32,
    iteration: u32,
) -> Option<String> {
    let source = animation.upcast::<SVGElement>();
    let by = parse_smil_color(&source.svg_attribute(&local_name!("by"))?)?;
    let base = target
        .svg_attribute(&LocalName::from(property))
        .as_deref()
        .and_then(parse_smil_color);
    let from = source.svg_attribute(&local_name!("from"));
    let mut start = match from.as_deref() {
        Some(from) => parse_smil_color(from)?,
        None => base?,
    };
    if from.is_some() && source.svg_attribute(&local_name!("additive")).as_deref() == Some("sum") {
        let base = base?;
        for index in 0..4 {
            start[index] += base[index];
        }
    }
    let (_, fraction) = animation.sample_value_interval(2, progress)?;
    let completed = if source.svg_attribute(&local_name!("accumulate")).as_deref() == Some("sum") {
        iteration as f32
    } else {
        0.0
    };
    let fraction = fraction + completed;
    let channel = |index: usize| {
        ((start[index] + by[index] * fraction).clamp(0.0, 1.0) * 255.0).round() as u8
    };
    let alpha = (start[3] + by[3] * fraction).clamp(0.0, 1.0);
    Some(format!(
        "rgb({} {} {} / {})",
        channel(0),
        channel(1),
        channel(2),
        alpha
    ))
}

impl SVGTimingBoundaryKind {
    pub(crate) fn order(self) -> u8 {
        match self {
            Self::End => 0,
            Self::Begin => 1,
            Self::Repeat(_) => 2,
        }
    }

    fn event_name(self) -> &'static str {
        match self {
            Self::End => "endEvent",
            Self::Begin => "beginEvent",
            Self::Repeat(_) => "repeatEvent",
        }
    }
}

/// <https://w3c.github.io/svgwg/specs/animations/#InterfaceSVGAnimationElement>
#[dom_struct]
pub(crate) struct SVGAnimationElement {
    svgelement: SVGElement,
    /// DOM calls add instance times without changing the reflected `begin`/`end` attributes.
    begin_instance_times: DomRefCell<Vec<f64>>,
    end_instance_times: DomRefCell<Vec<f64>>,
    /// Last interval boundaries processed by playback or a seek.
    last_begin_event: Cell<f64>,
    last_dispatched_begin_event: Cell<f64>,
    last_end_event: Cell<f64>,
    last_repeat_event: Cell<f64>,
    style_animation_id: Cell<u64>,
    style_target: MutNullableDom<Element>,
    class_style_effect: Cell<bool>,
    last_style_progress: Cell<Option<f64>>,
    last_style_signature: DomRefCell<Option<String>>,
}

impl SVGAnimationElement {
    pub(crate) fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> Self {
        Self {
            svgelement: SVGElement::new_inherited_with_state(
                ElementState::empty(),
                local_name,
                prefix,
                document,
            ),
            begin_instance_times: DomRefCell::new(Vec::new()),
            end_instance_times: DomRefCell::new(Vec::new()),
            last_begin_event: Cell::new(f64::NEG_INFINITY),
            last_dispatched_begin_event: Cell::new(f64::NEG_INFINITY),
            last_end_event: Cell::new(f64::NEG_INFINITY),
            last_repeat_event: Cell::new(f64::NEG_INFINITY),
            style_animation_id: Cell::new(crate::dom::animation::next_animation_id()),
            style_target: Default::default(),
            class_style_effect: Cell::new(false),
            last_style_progress: Cell::new(None),
            last_style_signature: Default::default(),
        }
    }

    pub(crate) fn current_time(&self) -> f64 {
        self.upcast::<SVGElement>()
            .viewport_element()
            .map_or(0.0, |svg| svg.animation_time())
    }

    pub(crate) fn target_element(
        &self,
        cx: &mut js::context::JSContext,
    ) -> Option<DomRoot<SVGElement>> {
        let href = self
            .upcast::<SVGElement>()
            .svg_attribute(&local_name!("href"))
            .or_else(|| {
                self.upcast::<Element>()
                    .get_attribute_string_value_with_namespace(&ns!(xlink), &local_name!("href"))
            });
        if let Some(href) = href {
            let id = href.strip_prefix('#')?;
            return self
                .owner_document()
                .GetElementById(cx, DOMString::from(id))
                .and_then(DomRoot::downcast::<SVGElement>);
        }
        self.upcast::<Node>()
            .GetParentElement()
            .and_then(DomRoot::downcast::<SVGElement>)
    }

    /// An animation is disabled when its own or an SVG ancestor's language
    /// condition does not match the user's language.
    pub(crate) fn passes_conditional_processing(&self) -> bool {
        let language = navigatorinfo::Language().to_ascii_lowercase();
        self.upcast::<Node>()
            .inclusive_ancestors(ShadowIncluding::No)
            .filter_map(DomRoot::downcast::<SVGElement>)
            .all(|element| {
                element
                    .svg_attribute(&local_name!("systemLanguage"))
                    .is_none_or(|value| {
                        value.split(',').any(|candidate| {
                            let candidate = candidate.trim().to_ascii_lowercase();
                            language == candidate || language.starts_with(&format!("{candidate}-"))
                        })
                    })
            })
    }

    pub(crate) fn simple_duration(&self) -> Option<f64> {
        self.upcast::<SVGElement>()
            .svg_attribute(&local_name!("dur"))
            .as_deref()
            .and_then(parse_clock_value)
            .filter(|duration| *duration >= 0.0)
    }

    fn begin_times(&self) -> Vec<f64> {
        let mut times = self
            .upcast::<SVGElement>()
            .svg_attribute(&local_name!("begin"))
            .map(|value| {
                value
                    .split(';')
                    .filter_map(parse_clock_value)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![0.0]);
        times.extend(self.begin_instance_times.borrow().iter().copied());
        times.sort_by(f64::total_cmp);
        times.dedup();
        if self
            .upcast::<SVGElement>()
            .svg_attribute(&local_name!("restart"))
            .as_deref() ==
            Some("never")
        {
            times.truncate(1);
        }
        times
    }

    /// Record a DOM event observed on an explicit eventbase, or on this
    /// animation's target when the eventbase is omitted.
    pub(crate) fn record_dom_event(
        &self,
        cx: &mut js::context::JSContext,
        source: &Element,
        event_name: &str,
    ) -> bool {
        if !self.passes_conditional_processing() {
            return false;
        }
        let explicit = source
            .get_id()
            .map(|id| format!("{}.{}", id.replace('.', "\\."), event_name));
        let implicit = self
            .GetTargetElement(cx)
            .is_some_and(|target| std::ptr::eq(target.upcast::<Element>(), source));
        let mut changed = false;
        for (attribute, is_begin) in [(local_name!("begin"), true), (local_name!("end"), false)] {
            let Some(specifiers) = self.upcast::<SVGElement>().svg_attribute(&attribute) else {
                continue;
            };
            for condition in specifiers.split(';').map(str::trim) {
                let offset = explicit
                    .as_deref()
                    .and_then(|prefix| timing_condition_offset(condition, prefix))
                    .or_else(|| {
                        implicit
                            .then(|| timing_condition_offset(condition, event_name))
                            .flatten()
                    });
                if let Some(offset) = offset {
                    let time = self.current_time() + offset;
                    if is_begin {
                        self.begin_instance_times.borrow_mut().push(time);
                    } else {
                        self.end_instance_times.borrow_mut().push(time);
                    }
                    changed = true;
                }
            }
        }
        changed
    }

    /// An eventbase or syncbase timing condition adds an instance time when
    /// the referenced animation reaches an interval boundary.
    pub(crate) fn record_timing_event(
        &self,
        source_id: &str,
        event_name: &str,
        repeat_index: Option<u32>,
        boundary: f64,
        source_time: f64,
        include_eventbase: bool,
    ) {
        let references = match event_name {
            "beginEvent" => vec!["beginEvent".to_owned(), "begin".to_owned()],
            "endEvent" => vec!["endEvent".to_owned(), "end".to_owned()],
            "repeatEvent" => {
                let mut references = vec!["repeatEvent".to_owned()];
                if let Some(index) = repeat_index {
                    references.push(format!("repeat({index})"));
                }
                references
            },
            _ => return,
        };
        // A dot inside an ID must be escaped in a timing reference.
        let escaped_id = source_id.replace('.', "\\.");
        for (attribute, is_begin) in [(local_name!("begin"), true), (local_name!("end"), false)] {
            let Some(specifiers) = self.upcast::<SVGElement>().svg_attribute(&attribute) else {
                continue;
            };
            for condition in specifiers.split(';').map(str::trim) {
                for reference in &references {
                    if !include_eventbase && reference.ends_with("Event") {
                        continue;
                    }
                    let prefix = format!("{escaped_id}.{reference}");
                    let offset = timing_condition_offset(condition, &prefix);
                    if let Some(offset) = offset {
                        let boundary_offset = if reference.ends_with("Event") {
                            0.0
                        } else {
                            boundary - source_time
                        };
                        let time = self.current_time() + boundary_offset + offset;
                        if is_begin {
                            self.begin_instance_times.borrow_mut().push(time);
                        } else {
                            self.end_instance_times.borrow_mut().push(time);
                        }
                        break;
                    }
                }
            }
        }
    }

    fn end_time(&self, begin: f64) -> f64 {
        let element = self.upcast::<SVGElement>();
        let repeat_count = element
            .svg_attribute(&local_name!("repeatCount"))
            .and_then(|value| {
                if value.trim() == "indefinite" {
                    Some(f64::INFINITY)
                } else {
                    value
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|count| count.is_finite())
                }
            })
            .filter(|count| *count > 0.0);
        let repeat_duration = element
            .svg_attribute(&local_name!("repeatDur"))
            .and_then(|value| {
                (value.trim() == "indefinite")
                    .then_some(f64::INFINITY)
                    .or_else(|| parse_clock_value(&value))
            })
            .filter(|duration| *duration >= 0.0);
        let simple_duration = self.simple_duration().unwrap_or(f64::INFINITY);
        let active_duration = if simple_duration == 0.0 {
            0.0
        } else if repeat_count.is_none() && repeat_duration.is_none() {
            simple_duration
        } else {
            (simple_duration * repeat_count.unwrap_or(f64::INFINITY))
                .min(repeat_duration.unwrap_or(f64::INFINITY))
        };
        let duration_end = begin + active_duration;
        let attribute_end = element
            .svg_attribute(&local_name!("end"))
            .into_iter()
            .flat_map(|value| {
                value
                    .split(';')
                    .filter_map(parse_clock_value)
                    .collect::<Vec<_>>()
            });
        let next_begin = self
            .begin_times()
            .into_iter()
            .find(|next| *next > begin)
            .unwrap_or(f64::INFINITY);
        attribute_end
            .chain(self.end_instance_times.borrow().iter().copied())
            .filter(|end| *end >= begin)
            .fold(duration_end.min(next_begin), f64::min)
    }

    /// The current or next interval, including intervals that have not begun yet.
    pub(crate) fn current_interval_start(&self) -> Option<f64> {
        let time = self.current_time();
        self.begin_times()
            .into_iter()
            .find(|begin| self.end_time(*begin) > time)
    }

    pub(crate) fn active_interval_at(&self, time: f64) -> Option<f64> {
        self.begin_times()
            .into_iter()
            .filter(|begin| *begin <= time && time < self.end_time(*begin))
            .last()
    }

    /// The latest interval start determines this effect's SMIL composition order.
    pub(crate) fn last_begin_time(&self) -> Option<f64> {
        let time = self.current_time();
        self.begin_times()
            .into_iter()
            .filter(|begin| *begin <= time)
            .last()
    }

    /// A seek from within another timing event can consume this animation's
    /// begin boundary before its pending DOM event is dispatched.
    pub(crate) fn take_pending_begin_event(&self, interval_begin: f64) -> bool {
        if self.last_begin_event.get() != interval_begin ||
            self.last_dispatched_begin_event.get() >= interval_begin
        {
            return false;
        }
        self.last_dispatched_begin_event.set(interval_begin);
        true
    }

    pub(crate) fn mark_begin_event_dispatched(&self) {
        self.last_dispatched_begin_event
            .set(self.last_begin_event.get());
    }

    /// Progress within the current repeat and the number of completed repeats.
    /// At an exact active end, freezing samples the end of the final repeat.
    pub(crate) fn sample_iteration(&self) -> Option<(f32, u32)> {
        let time = self.current_time();
        let begin = self
            .begin_times()
            .into_iter()
            .filter(|begin| *begin <= time)
            .last()?;
        let duration = self.simple_duration().unwrap_or(f64::INFINITY);
        if duration == 0.0 {
            return None;
        }
        let end = self.end_time(begin);
        if begin < 0.0 && end <= 0.0 {
            return None;
        }
        if time >= end &&
            self.upcast::<SVGElement>()
                .svg_attribute(&local_name!("fill"))
                .as_deref() !=
                Some("freeze")
        {
            return None;
        }
        let elapsed = (time.min(end) - begin).max(0.0);
        let position = elapsed / duration;
        let completed = position.floor();
        if time >= end && elapsed > 0.0 && position - completed < 1e-10 {
            return Some((1.0, (completed - 1.0) as u32));
        }
        Some(((position - completed) as f32, completed as u32))
    }

    /// Progress for simple value interpolation, including a frozen final value.
    pub(crate) fn sample_progress(&self) -> Option<f32> {
        self.sample_iteration().map(|(progress, _)| progress)
    }

    /// Strings, booleans, and other non-interpolable values occupy equal parts
    /// of the simple duration unless valid discrete key times override them.
    pub(crate) fn sample_discrete_index(&self, count: usize) -> Option<usize> {
        let progress = self.sample_progress()?;
        if count == 0 {
            return None;
        }
        if count == 1 || self.simple_duration().is_none() {
            return Some(0);
        }
        if let Some(times) = self
            .upcast::<SVGElement>()
            .svg_attribute(&local_name!("keyTimes"))
        {
            let times: Option<Vec<f32>> = trim_one_trailing_semicolon(&times)
                .split(';')
                .map(|time| time.trim().parse::<f32>().ok())
                .collect();
            let times = times?;
            if times.len() != count ||
                times.first() != Some(&0.0) ||
                times.iter().any(|time| !(0.0..=1.0).contains(time)) ||
                times.windows(2).any(|pair| pair[0] > pair[1])
            {
                return None;
            }
            return Some(
                times
                    .partition_point(|time| *time <= progress)
                    .saturating_sub(1),
            );
        }
        Some(((progress * count as f32) as usize).min(count - 1))
    }

    fn calculation_mode(&self) -> String {
        self.upcast::<SVGElement>()
            .svg_attribute(&local_name!("calcMode"))
            .unwrap_or_else(|| "linear".to_owned())
    }

    /// The positions and easing functions of the values in a SMIL effect.
    fn value_timing(&self, count: usize) -> Option<(Vec<f64>, Vec<TimingFunction>)> {
        self.value_timing_for_mode(count, &self.calculation_mode())
    }

    fn value_timing_for_mode(
        &self,
        count: usize,
        mode: &str,
    ) -> Option<(Vec<f64>, Vec<TimingFunction>)> {
        if count < 2 {
            return None;
        }
        let element = self.upcast::<SVGElement>();
        let offsets = if mode != "paced" {
            if let Some(times) = element.svg_attribute(&local_name!("keyTimes")) {
                let times = trim_one_trailing_semicolon(&times);
                let offsets: Option<Vec<f64>> = times
                    .split(';')
                    .map(|time| time.trim().parse::<f64>().ok())
                    .collect();
                let offsets = offsets?;
                if offsets.len() != count ||
                    offsets.first() != Some(&0.0) ||
                    offsets.iter().any(|time| !(0.0..=1.0).contains(time)) ||
                    offsets.windows(2).any(|pair| pair[0] > pair[1]) ||
                    (mode != "discrete" && offsets.last() != Some(&1.0))
                {
                    return None;
                }
                offsets
            } else if mode == "discrete" {
                (0..count)
                    .map(|index| index as f64 / count as f64)
                    .collect()
            } else {
                (0..count)
                    .map(|index| index as f64 / (count - 1) as f64)
                    .collect()
            }
        } else {
            let paced_numbers: Option<Vec<f64>> = element
                .svg_attribute(&local_name!("values"))
                .map(|values| {
                    trim_one_trailing_semicolon(&values)
                        .split(';')
                        .map(|value| value.trim().parse::<f64>().ok())
                        .collect::<Option<Vec<f64>>>()
                })
                .flatten()
                .filter(|values| {
                    values.len() == count && values.iter().all(|value| value.is_finite())
                });
            if let Some(values) = paced_numbers {
                let mut offsets = vec![0.0_f64];
                for pair in values.windows(2) {
                    offsets.push(offsets.last().unwrap() + (pair[1] - pair[0]).abs());
                }
                let total = *offsets.last().unwrap();
                if total.is_finite() && total > 0.0 {
                    offsets
                        .into_iter()
                        .map(|distance| distance / total)
                        .collect()
                } else {
                    (0..count)
                        .map(|index| index as f64 / (count - 1) as f64)
                        .collect()
                }
            } else {
                (0..count)
                    .map(|index| index as f64 / (count - 1) as f64)
                    .collect()
            }
        };
        let functions = if mode == "discrete" {
            vec![TimingFunction::Steps(1, StepPosition::JumpEnd); count - 1]
        } else if mode == "spline" {
            let splines = element.svg_attribute(&local_name!("keySplines"))?;
            let splines = trim_one_trailing_semicolon(&splines);
            let functions: Option<Vec<_>> = splines
                .split(';')
                .map(|spline| {
                    let numbers: Option<Vec<f32>> = spline
                        .split(|character: char| {
                            character.is_ascii_whitespace() || character == ','
                        })
                        .filter(|part| !part.is_empty())
                        .map(|part| part.parse::<f32>().ok())
                        .collect();
                    let numbers = numbers?;
                    let [x1, y1, x2, y2] = numbers.as_slice() else {
                        return None;
                    };
                    if !numbers.iter().all(|number| (0.0..=1.0).contains(number)) {
                        return None;
                    }
                    Some(TimingFunction::CubicBezier {
                        x1: *x1,
                        y1: *y1,
                        x2: *x2,
                        y2: *y2,
                    })
                })
                .collect();
            let functions = functions?;
            if functions.len() != count - 1 {
                return None;
            }
            functions
        } else {
            vec![TimingFunction::Keyword(TimingKeyword::Linear); count - 1]
        };
        Some((offsets, functions))
    }

    /// Locate the current value pair and return its eased interpolation fraction.
    pub(crate) fn sample_value_interval(
        &self,
        count: usize,
        progress: f32,
    ) -> Option<(usize, f32)> {
        let (offsets, functions) = self.value_timing(count)?;
        let progress = f64::from(progress);
        if self.calculation_mode() == "discrete" {
            let index = offsets
                .partition_point(|offset| *offset <= progress)
                .saturating_sub(1)
                .min(count - 1);
            return Some(if index == count - 1 {
                (count - 2, 1.0)
            } else {
                (index, 0.0)
            });
        }
        let index = offsets
            .partition_point(|offset| *offset <= progress)
            .saturating_sub(1)
            .min(count - 2);
        let distance = offsets[index + 1] - offsets[index];
        let fraction = if distance > 0.0 {
            ((progress - offsets[index]) / distance).clamp(0.0, 1.0)
        } else {
            1.0
        };
        Some((
            index,
            functions[index].calculate_output(fraction, BeforeFlag::Unset, 1e-6) as f32,
        ))
    }

    /// Whether a `<set>` effect applies at the current SVG time.
    pub(crate) fn set_is_active(&self) -> bool {
        let time = self.current_time();
        let Some(begin) = self
            .begin_times()
            .into_iter()
            .filter(|begin| *begin <= time)
            .last()
        else {
            return false;
        };
        if begin < 0.0 && self.end_time(begin) <= 0.0 {
            return false;
        }
        time < self.end_time(begin) ||
            self.upcast::<SVGElement>()
                .svg_attribute(&local_name!("fill"))
                .as_deref() ==
                Some("freeze")
    }

    fn remove_style_effect(&self, cx: &mut js::context::JSContext) {
        if let Some(target) = self.style_target.get() {
            let node = target.upcast::<Node>();
            if self.class_style_effect.get() {
                target.set_animated_class(cx, self.style_animation_id.get(), None);
            } else {
                self.owner_document()
                    .animation_manager()
                    .sets()
                    .remove_script_animation(
                        &AnimationSetKey::new_for_non_pseudo(node.to_opaque()),
                        self.style_animation_id.get(),
                    );
            }
            node.dirty(cx.no_gc(), NodeDamage::Style);
            self.style_target.set(None);
        }
        self.class_style_effect.set(false);
        self.last_style_progress.set(None);
        *self.last_style_signature.borrow_mut() = None;
    }

    /// Give CSS presentation properties animated by SMIL to the same style
    /// compositor that samples Web Animations. The SVG attribute remains the
    /// base value, including while its computed style is animated.
    pub(crate) fn update_style_effect(&self, cx: &mut js::context::JSContext) {
        if !self.upcast::<Node>().is_connected() || !self.passes_conditional_processing() {
            self.remove_style_effect(cx);
            return;
        }
        let Some(target) = self.GetTargetElement(cx) else {
            self.remove_style_effect(cx);
            return;
        };
        let element = self.upcast::<SVGElement>();
        let Some(property) = element.svg_attribute(&local_name!("attributeName")) else {
            self.remove_style_effect(cx);
            return;
        };
        let is_set = self.upcast::<Element>().local_name() == &local_name!("set");
        let sample = if is_set {
            self.set_is_active().then_some((1.0, 0))
        } else {
            self.sample_iteration()
                .map(|(progress, iteration)| (f64::from(progress), iteration))
        };
        let Some((progress, iteration)) = sample else {
            self.remove_style_effect(cx);
            return;
        };
        if property == "class" {
            if element
                .svg_attribute(&local_name!("attributeType"))
                .as_deref() ==
                Some("CSS")
            {
                self.remove_style_effect(cx);
                return;
            }
            let value = if is_set {
                element.svg_attribute(&local_name!("to"))
            } else if let Some(values) = element.svg_attribute(&local_name!("values")) {
                let values: Vec<_> = trim_one_trailing_semicolon(&values)
                    .split(';')
                    .map(|value| value.trim().to_owned())
                    .collect();
                self.sample_discrete_index(values.len())
                    .and_then(|index| values.get(index).cloned())
            } else if let Some(to) = element.svg_attribute(&local_name!("to")) {
                let from = element
                    .svg_attribute(&local_name!("from"))
                    .unwrap_or_else(|| {
                        target
                            .upcast::<Element>()
                            .get_attribute_string_value(&local_name!("class"))
                            .unwrap_or_default()
                    });
                self.sample_discrete_index(2)
                    .map(|index| if index == 0 { from } else { to })
            } else {
                None
            };
            let Some(value) = value else {
                self.remove_style_effect(cx);
                return;
            };
            let target = target.upcast::<Element>();
            if self.style_target.get().as_deref() != Some(target) || !self.class_style_effect.get()
            {
                self.remove_style_effect(cx);
            }
            target.set_animated_class(cx, self.style_animation_id.get(), Some(value));
            self.style_target.set(Some(target));
            self.class_style_effect.set(true);
            return;
        }
        if self.class_style_effect.get() {
            self.remove_style_effect(cx);
        }
        let mut values: Vec<Option<String>> = if is_set {
            match element.svg_attribute(&local_name!("to")) {
                Some(to) => vec![Some(to.clone()), Some(to)],
                None => Vec::new(),
            }
        } else if let Some(values) = element.svg_attribute(&local_name!("values")) {
            let mut values: Vec<_> = trim_one_trailing_semicolon(&values)
                .split(';')
                .map(|value| Some(value.trim().to_owned()))
                .collect();
            if values.len() == 1 {
                values.push(values[0].clone());
            }
            values
        } else {
            match (
                element.svg_attribute(&local_name!("from")),
                element.svg_attribute(&local_name!("to")),
            ) {
                (Some(from), Some(to)) => vec![Some(from), Some(to)],
                (None, Some(to)) => vec![None, Some(to)],
                _ => Vec::new(),
            }
        };
        let mut sampled_color = false;
        if !is_set &&
            values.is_empty() &&
            matches!(
                property.as_str(),
                "color" | "fill" | "stroke" | "flood-color" | "lighting-color" | "stop-color"
            ) &&
            element.svg_attribute(&local_name!("by")).is_some() &&
            let Some(color) =
                sampled_smil_color_by(self, &target, &property, progress as f32, iteration)
        {
            values = vec![Some(color.clone()), Some(color)];
            sampled_color = true;
        }
        let mut by_only = false;
        let mut accumulation_path = None;
        if property == "d" && !is_set {
            let parse_path = |value: &str| {
                let (path, valid) = SVGPathData::parse_bytes(value.as_bytes());
                valid.then_some(path)
            };
            if values.is_empty() &&
                let Some(by) = element.svg_attribute(&local_name!("by"))
            {
                let from = element.svg_attribute(&local_name!("from"));
                // A by-only path must be addable to the underlying path even
                // before the sampled value is given to the style compositor.
                if let Some(by_path) = parse_path(&by) &&
                    !by_path.commands().is_empty() &&
                    (from.is_some() ||
                        target
                            .downcast::<SVGElement>()
                            .and_then(|target| target.svg_attribute(&local_name!("d")))
                            .as_deref()
                            .and_then(parse_path)
                            .is_some_and(|base| {
                                !base.commands().is_empty() &&
                                    base.animate(&by_path, Procedure::Add).is_ok()
                            })) &&
                    let Some(start) = from
                        .as_deref()
                        .map(parse_path)
                        .unwrap_or_else(|| by_path.to_animated_zero().ok()) &&
                    !start.commands().is_empty()
                {
                    by_only = from.is_none();
                    let end = if by_only {
                        by_path.clone()
                    } else if let Ok(end) = start.animate(&by_path, Procedure::Add) {
                        end
                    } else {
                        self.remove_style_effect(cx);
                        return;
                    };
                    if by_only {
                        accumulation_path = Some(by_path);
                    }
                    values = vec![
                        Some(start.to_css_string().trim_matches('"').to_owned()),
                        Some(end.to_css_string().trim_matches('"').to_owned()),
                    ];
                }
            }
            if iteration > 0 &&
                element.svg_attribute(&local_name!("accumulate")).as_deref() == Some("sum") &&
                !(element.svg_attribute(&local_name!("from")).is_none() &&
                    element.svg_attribute(&local_name!("to")).is_some())
            {
                let final_path = accumulation_path.or_else(|| {
                    values
                        .last()
                        .and_then(Option::as_deref)
                        .and_then(parse_path)
                });
                let Some(final_path) = final_path else {
                    self.remove_style_effect(cx);
                    return;
                };
                let accumulated: Option<Vec<_>> = values
                    .iter()
                    .map(|value| {
                        let path = parse_path(value.as_deref()?)?;
                        let path = final_path
                            .animate(
                                &path,
                                Procedure::Accumulate {
                                    count: u64::from(iteration),
                                },
                            )
                            .ok()?;
                        Some(Some(path.to_css_string().trim_matches('"').to_owned()))
                    })
                    .collect();
                let Some(accumulated) = accumulated else {
                    self.remove_style_effect(cx);
                    return;
                };
                values = accumulated;
            }
        }
        if values.len() < 2 {
            self.remove_style_effect(cx);
            return;
        }
        // SVG strings use discrete SMIL intervals even when calcMode requests
        // interpolation. CSS visibility has special CSS interpolation rules,
        // so its SMIL values must use the discrete interval positions here.
        let mode = if property == "visibility" {
            "discrete".to_owned()
        } else {
            self.calculation_mode()
        };
        let Some((offsets, functions)) = self.value_timing_for_mode(values.len(), &mode) else {
            self.remove_style_effect(cx);
            return;
        };
        let additive = !is_set &&
            !sampled_color &&
            (by_only ||
                element.svg_attribute(&local_name!("additive")).as_deref() == Some("sum"));
        // Empty path endpoints cannot compose additively with a nonempty base.
        if property == "d" &&
            additive &&
            values
                .iter()
                .all(|value| value.as_ref().is_some_and(|value| value.trim().is_empty()))
        {
            self.remove_style_effect(cx);
            return;
        }
        let signature = format!("{property}:{values:?}:{additive}:{offsets:?}:{functions:?}");
        let target = target.upcast::<Element>();
        if self.style_target.get().as_deref() == Some(target) &&
            self.last_style_progress.get() == Some(progress) &&
            self.last_style_signature.borrow().as_deref() == Some(signature.as_str())
        {
            return;
        }

        let document = self.owner_document();
        let keyframes: Option<Vec<_>> = values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let declarations = if let Some(value) = value {
                    let css_value = if property == "d" &&
                        element
                            .svg_attribute(&local_name!("attributeType"))
                            .as_deref() ==
                            Some("CSS") &&
                        matches!(
                            value.trim(),
                            "inherit" | "initial" | "unset" | "revert" | "revert-layer"
                        ) {
                        value.trim().to_owned()
                    } else if property == "d" {
                        format!(
                            "path(\"{}\")",
                            value.replace('\\', "\\\\").replace('"', "\\\"")
                        )
                    } else if matches!(
                        property.as_str(),
                        "x" | "y" |
                            "cx" |
                            "cy" |
                            "r" |
                            "rx" |
                            "ry" |
                            "width" |
                            "height" |
                            "stroke-width" |
                            "stroke-dashoffset" |
                            "stroke-dasharray" |
                            "pathLength" |
                            "path-length" |
                            "font-size" |
                            "letter-spacing" |
                            "word-spacing"
                    ) && value
                        .trim()
                        .parse::<f64>()
                        .is_ok_and(|number| number.is_finite())
                    {
                        format!("{}px", value.trim())
                    } else {
                        value.clone()
                    };
                    let css_property = if property == "pathLength" {
                        "path-length"
                    } else {
                        property.as_str()
                    };
                    let block = parse_style_attribute(
                        &format!("{css_property}: {css_value}"),
                        &UrlExtraData(document.base_url().get_arc()),
                        None,
                        document.quirks_mode(),
                        CssRuleType::Style,
                    );
                    if block.normal_declaration_iter().next().is_none() {
                        return None;
                    }
                    block
                } else {
                    PropertyDeclarationBlock::new()
                };
                Some(ScriptKeyframe {
                    offset: offsets[index],
                    timing_function: functions
                        .get(index)
                        .cloned()
                        .unwrap_or(TimingFunction::Keyword(TimingKeyword::Linear)),
                    composite: None,
                    declarations: ServoArc::new(declarations),
                })
            })
            .collect();
        let Some(mut keyframes) = keyframes else {
            self.remove_style_effect(cx);
            return;
        };
        if offsets.last().is_some_and(|offset| *offset < 1.0) {
            let mut final_keyframe = keyframes.last().unwrap().clone();
            final_keyframe.offset = 1.0;
            keyframes.push(final_keyframe);
        }
        if self.style_target.get().as_deref() != Some(target) {
            self.remove_style_effect(cx);
        }
        let node = target.upcast::<Node>();
        let composite = if additive {
            AnimationComposition::Add
        } else {
            AnimationComposition::Replace
        };
        document.animation_manager().sets().update_script_animation(
            AnimationSetKey::new_for_non_pseudo(node.to_opaque()),
            ScriptAnimation::new(
                self.style_animation_id.get(),
                ServoArc::new(keyframes),
                composite,
                Some(progress),
                false,
                false,
            ),
        );
        node.set_flag(NodeFlags::HAS_EVER_ANIMATED, true);
        node.dirty(cx.no_gc(), NodeDamage::Style);
        self.style_target.set(Some(target));
        self.last_style_progress.set(Some(progress));
        *self.last_style_signature.borrow_mut() = Some(signature);
    }

    fn next_repeat_boundary(&self, begin: f64, through: f64) -> Option<(f64, u32)> {
        let duration = self.simple_duration().filter(|duration| *duration > 0.0)?;
        let end = self.end_time(begin);
        let last = self.last_repeat_event.get();
        let mut index = if last <= begin {
            1
        } else {
            (((last - begin) / duration).floor() as u32).saturating_add(1)
        };
        for _ in 0..2 {
            let boundary = begin + duration * f64::from(index);
            if boundary > last && boundary <= through && boundary < end {
                return Some((boundary, index));
            }
            if boundary > through || boundary >= end {
                return None;
            }
            index = index.saturating_add(1);
        }
        None
    }

    /// The earliest unprocessed interval boundary through the seek time.
    pub(crate) fn next_timing_boundary_before(
        &self,
        through: f64,
    ) -> Option<(f64, SVGTimingBoundaryKind)> {
        let mut next: Option<(f64, SVGTimingBoundaryKind)> = None;
        for begin in self.begin_times() {
            let mut candidates = vec![
                (
                    begin,
                    SVGTimingBoundaryKind::Begin,
                    self.last_begin_event.get(),
                ),
                (
                    self.end_time(begin),
                    SVGTimingBoundaryKind::End,
                    self.last_end_event.get(),
                ),
            ];
            if let Some((boundary, index)) = self.next_repeat_boundary(begin, through) {
                candidates.push((
                    boundary,
                    SVGTimingBoundaryKind::Repeat(index),
                    self.last_repeat_event.get(),
                ));
            }
            for (boundary, kind, last) in candidates {
                if !boundary.is_finite() || boundary > through || boundary <= last {
                    continue;
                }
                let candidate = (boundary, kind);
                if next.is_none_or(|current| {
                    candidate
                        .0
                        .total_cmp(&current.0)
                        .then_with(|| candidate.1.order().cmp(&current.1.order())) ==
                        std::cmp::Ordering::Less
                }) {
                    next = Some(candidate);
                }
            }
        }
        next
    }

    pub(crate) fn process_timing_boundary(
        &self,
        boundary: f64,
        kind: SVGTimingBoundaryKind,
        include_eventbase: bool,
    ) {
        match kind {
            SVGTimingBoundaryKind::Begin => self.last_begin_event.set(boundary),
            SVGTimingBoundaryKind::End => self.last_end_event.set(boundary),
            SVGTimingBoundaryKind::Repeat(_) => self.last_repeat_event.set(boundary),
        }
        if let Some(id) = self.upcast::<Element>().get_id() {
            self.owner_document().notify_smil_timing_event(
                &id,
                kind.event_name(),
                match kind {
                    SVGTimingBoundaryKind::Repeat(index) => Some(index),
                    _ => None,
                },
                boundary,
                self.current_time(),
                include_eventbase,
            );
        }
    }

    /// Time until the next undispatched timing event on the SVG clock.
    pub(crate) fn next_timing_event_delay(&self) -> Option<Duration> {
        if !self.passes_conditional_processing() {
            return None;
        }
        let root = self.upcast::<SVGElement>().viewport_element()?;
        if root.animations_are_paused() {
            return None;
        }
        let time = self.current_time();
        self.next_timing_boundary_before(f64::INFINITY)
            .map(|(boundary, _)| Duration::from_secs_f64((boundary - time).clamp(0.0, 86400.0)))
    }

    /// The next interval boundary that playback has reached. The document
    /// processes boundaries across animations in timeline order, since one
    /// animation can create a boundary for an earlier sibling.
    pub(crate) fn next_due_timing_boundary(&self) -> Option<(f64, SVGTimingBoundaryKind)> {
        if !self.passes_conditional_processing() {
            return None;
        }
        let Some(root) = self.upcast::<SVGElement>().viewport_element() else {
            return None;
        };
        if root.animations_are_paused() || !self.upcast::<Node>().is_connected() {
            return None;
        }
        self.next_timing_boundary_before(self.current_time())
    }

    pub(crate) fn dispatch_timing_boundary(
        &self,
        cx: &mut js::context::JSContext,
        boundary: f64,
        kind: SVGTimingBoundaryKind,
    ) {
        if matches!(kind, SVGTimingBoundaryKind::Begin) {
            // SMIL effects that begin later have higher priority than effects
            // already active on the same target.
            self.remove_style_effect(cx);
            self.style_animation_id
                .set(crate::dom::animation::next_animation_id());
        }
        self.process_timing_boundary(boundary, kind, true);
        self.update_style_effect(cx);
        if matches!(kind, SVGTimingBoundaryKind::Begin) {
            self.mark_begin_event_dispatched();
        }
        self.upcast::<EventTarget>()
            .fire_event(cx, Atom::from(kind.event_name()));
    }
}

/// Sample a numeric child animation whose `by` value has its own grammar.
pub(crate) fn sample_numeric_animation_with_by(
    target: &SVGElement,
    attribute: &LocalName,
    base: f32,
    parse: impl Fn(&str) -> Option<f32>,
    parse_by: impl Fn(&str) -> Option<f32>,
) -> f32 {
    let animations = target
        .upcast::<Node>()
        .child_elements()
        .filter_map(DomRoot::downcast::<SVGAnimationElement>)
        .collect();
    sample_numeric_animations(animations, attribute, base, parse, parse_by)
}

/// Sample all animations targeting an SVG attribute, including external href targets.
pub(crate) fn sample_targeted_numeric_animation(
    cx: &mut js::context::JSContext,
    target: &SVGElement,
    attribute: &LocalName,
    base: f32,
    parse: impl Fn(&str) -> Option<f32>,
) -> f32 {
    sample_targeted_numeric_animation_with_by(cx, target, attribute, base, &parse, &parse)
}

/// Sample all targeted numeric animations with a separate grammar for `by`.
pub(crate) fn sample_targeted_numeric_animation_with_by(
    cx: &mut js::context::JSContext,
    target: &SVGElement,
    attribute: &LocalName,
    base: f32,
    parse: impl Fn(&str) -> Option<f32>,
    parse_by: impl Fn(&str) -> Option<f32>,
) -> f32 {
    let animations = target
        .owner_document()
        .smil_animations_targeting(cx, target);
    sample_numeric_animations(animations, attribute, base, parse, parse_by)
}

/// `parse` handles the attribute's number grammar, including compound
/// attributes whose DOM objects expose one component each.
fn sample_numeric_animations(
    animations: Vec<DomRoot<SVGAnimationElement>>,
    attribute: &LocalName,
    base: f32,
    parse: impl Fn(&str) -> Option<f32>,
    parse_by: impl Fn(&str) -> Option<f32>,
) -> f32 {
    let mut result = base;
    for animation in animations {
        let child = animation.upcast::<Element>();
        if child.local_name() != &local_name!("animate") &&
            child.local_name() != &local_name!("set")
        {
            continue;
        }
        let element = animation.upcast::<SVGElement>();
        if element
            .svg_attribute(&local_name!("attributeName"))
            .as_deref() !=
            Some(attribute.as_ref())
        {
            continue;
        }
        if child.local_name() == &local_name!("set") {
            if animation.set_is_active() &&
                let Some(to) = element
                    .svg_attribute(&local_name!("to"))
                    .as_deref()
                    .and_then(&parse)
            {
                result = to;
            }
            continue;
        }
        let Some((progress, iteration)) = animation.sample_iteration() else {
            continue;
        };
        let additive = element.svg_attribute(&local_name!("additive")).as_deref() == Some("sum");
        let accumulate =
            element.svg_attribute(&local_name!("accumulate")).as_deref() == Some("sum");
        if let Some(values) = element.svg_attribute(&local_name!("values")) {
            let values: Option<Vec<_>> = trim_one_trailing_semicolon(&values)
                .split(';')
                .map(&parse)
                .collect();
            let Some(values) = values.filter(|values| !values.is_empty()) else {
                continue;
            };
            let mut effect = if values.len() == 1 {
                values[0]
            } else {
                let Some((index, fraction)) =
                    animation.sample_value_interval(values.len(), progress)
                else {
                    continue;
                };
                values[index] + (values[index + 1] - values[index]) * fraction
            };
            if accumulate {
                effect += values[values.len() - 1] * iteration as f32;
            }
            result = if additive { result + effect } else { effect };
            continue;
        }
        let from_attribute = element.svg_attribute(&local_name!("from"));
        let to_attribute = element.svg_attribute(&local_name!("to"));
        let by_attribute = element.svg_attribute(&local_name!("by"));
        let from = from_attribute.as_deref().and_then(&parse);
        let to = to_attribute.as_deref().and_then(&parse);
        let by = by_attribute.as_deref().and_then(&parse_by);
        if (from_attribute.is_some() && from.is_none()) ||
            (to_attribute.is_some() && to.is_none()) ||
            (by_attribute.is_some() && by.is_none())
        {
            continue;
        }
        let (start, end, additive) = match (from, to, by) {
            (Some(from), Some(to), _) => (from, to, additive),
            (Some(from), None, Some(by)) => (from, from + by, additive),
            (None, None, Some(by)) => (0.0, by, true),
            (None, Some(to), _) => (result, to, false),
            _ => continue,
        };
        let Some((_, fraction)) = animation.sample_value_interval(2, progress) else {
            continue;
        };
        let mut effect = start + (end - start) * fraction;
        if accumulate && (from.is_some() || by.is_some()) {
            effect += end * iteration as f32;
        }
        result = if additive { result + effect } else { effect };
    }
    result
}

/// The final semicolon terminates a list; earlier empty entries remain values.
pub(crate) fn trim_one_trailing_semicolon(value: &str) -> &str {
    let value = value.trim();
    value.strip_suffix(';').unwrap_or(value)
}

fn timing_condition_offset(condition: &str, prefix: &str) -> Option<f64> {
    let offset = condition.strip_prefix(prefix)?.trim();
    if offset.is_empty() {
        Some(0.0)
    } else if matches!(offset.chars().next(), Some('+' | '-')) {
        parse_clock_value(offset)
    } else {
        None
    }
}

/// Parse a SMIL clock value expressed as a number of seconds, with a unit, or as hh:mm:ss.
pub(crate) fn parse_clock_value(value: &str) -> Option<f64> {
    let value = value.trim();
    let (sign, value) = if let Some(value) = value.strip_prefix('-') {
        (-1.0, value)
    } else if let Some(value) = value.strip_prefix('+') {
        (1.0, value)
    } else {
        (1.0, value)
    };
    fn digits(value: &str) -> bool {
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
    }
    fn decimal(value: &str) -> Option<f64> {
        let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
        if !digits(whole) || (value.contains('.') && !digits(fraction)) {
            return None;
        }
        value.parse().ok()
    }
    fn minute(value: &str) -> Option<f64> {
        (value.len() == 2 && digits(value))
            .then(|| value.parse::<u8>().ok())
            .flatten()
            .filter(|value| *value < 60)
            .map(f64::from)
    }
    fn second(value: &str) -> Option<f64> {
        let whole = value.split_once('.').map_or(value, |(whole, _)| whole);
        (whole.len() == 2).then_some(())?;
        let seconds = decimal(value)?;
        (seconds < 60.0).then_some(seconds)
    }
    let seconds = if value.contains(':') {
        let parts = value.split(':').collect::<Vec<_>>();
        match parts.as_slice() {
            [minutes, seconds] => minute(minutes)? * 60.0 + second(seconds)?,
            [hours, minutes, seconds] => {
                (digits(hours)).then_some(())?;
                hours.parse::<f64>().ok()? * 3600.0 + minute(minutes)? * 60.0 + second(seconds)?
            },
            _ => return None,
        }
    } else {
        let (number, scale) = if let Some(number) = value.strip_suffix("ms") {
            (number, 0.001)
        } else if let Some(number) = value.strip_suffix("min") {
            (number, 60.0)
        } else if let Some(number) = value.strip_suffix('h') {
            (number, 3600.0)
        } else if let Some(number) = value.strip_suffix('s') {
            (number, 1.0)
        } else {
            (value, 1.0)
        };
        decimal(number)? * scale
    };
    seconds.is_finite().then_some(seconds * sign)
}

impl VirtualMethods for SVGAnimationElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGElement>() as &dyn VirtualMethods)
    }

    fn bind_to_tree(&self, cx: &mut js::context::JSContext, context: &BindContext) {
        if let Some(super_type) = self.super_type() {
            super_type.bind_to_tree(cx, context);
        }
        if context.tree_connected {
            self.owner_document().register_smil_animation(self);
        }
    }

    fn unbind_from_tree(&self, cx: &mut js::context::JSContext, context: &UnbindContext) {
        if context.tree_connected {
            self.remove_style_effect(cx);
            self.owner_document().unregister_smil_animation(self);
        }
        if let Some(super_type) = self.super_type() {
            super_type.unbind_from_tree(cx, context);
        }
    }
}

impl SVGAnimationElementMethods<crate::DomTypeHolder> for SVGAnimationElement {
    fn GetTargetElement(&self, cx: &mut js::context::JSContext) -> Option<DomRoot<SVGElement>> {
        self.target_element(cx)
    }

    fn GetStartTime(&self) -> Fallible<Finite<f32>> {
        self.current_interval_start()
            .and_then(|time| Finite::new(time as f32))
            .ok_or_else(|| Error::InvalidState(Some("Animation has no current interval".into())))
    }

    fn GetCurrentTime(&self) -> Finite<f32> {
        Finite::wrap(self.current_time() as f32)
    }

    fn GetSimpleDuration(&self) -> Fallible<Finite<f32>> {
        self.simple_duration()
            .and_then(|duration| Finite::new(duration as f32))
            .ok_or_else(|| Error::NotSupported(Some("Animation has no simple duration".into())))
    }

    fn BeginElement(&self) {
        self.BeginElementAt(Finite::wrap(0.0));
    }

    fn BeginElementAt(&self, offset: Finite<f32>) {
        let begin = self.current_time() + *offset as f64;
        if *offset == 0.0 {
            // An immediate restart clears end times created by earlier DOM
            // calls, including an endElement() on an inactive animation.
            self.end_instance_times
                .borrow_mut()
                .retain(|end| *end > begin);
        }
        self.begin_instance_times.borrow_mut().push(begin);
    }

    fn EndElement(&self) {
        self.EndElementAt(Finite::wrap(0.0));
    }

    fn EndElementAt(&self, offset: Finite<f32>) {
        self.end_instance_times
            .borrow_mut()
            .push(self.current_time() + *offset as f64);
    }

    fn RequiredExtensions(&self, cx: &mut js::context::JSContext) -> DomRoot<SVGStringList> {
        self.upcast::<SVGElement>()
            .string_list(cx, "requiredExtensions", false)
    }

    fn SystemLanguage(&self, cx: &mut js::context::JSContext) -> DomRoot<SVGStringList> {
        self.upcast::<SVGElement>()
            .string_list(cx, "systemLanguage", true)
    }
}
