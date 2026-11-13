/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use icu_segmenter::GraphemeClusterSegmenter;
use layout_api::LayoutNode;
use script::layout_dom::ServoLayoutNode;
use style::counter_style::{
    AdditiveTuple, CounterBound, CounterRanges, CounterStyle, Symbol, SymbolsType, System,
};
use style::properties::LonghandId;
use style::properties::longhands::list_style_type::computed_value::T as ListStyleType;
use style::stylesheets::Origin;
use style::values::computed::Image;
use style::values::generics::counters::Content;
use stylo_atoms::atom;

use crate::context::LayoutContext;
use crate::dom_traversal::{
    NodeAndStyleInfo, PseudoElementContentItem, generate_pseudo_element_content,
};
use crate::replaced::ReplacedContents;

/// <https://drafts.csswg.org/css-lists/#content-property>
pub(crate) fn make_marker<'dom>(
    context: &LayoutContext,
    info: &NodeAndStyleInfo<'dom>,
) -> Option<(NodeAndStyleInfo<'dom>, Vec<PseudoElementContentItem>)> {
    let marker_info =
        info.with_pseudo_element(context, style::selector_parser::PseudoElement::Marker)?;
    let style = &marker_info.style;
    let list_style = style.get_list();

    // https://drafts.csswg.org/css-lists/#marker-image
    let marker_image = || match &list_style.list_style_image {
        Image::Url(url) => Some(vec![
            PseudoElementContentItem::Replaced(ReplacedContents::from_image_url(
                marker_info.node,
                context,
                url,
            )?),
            PseudoElementContentItem::Text(" ".into()),
        ]),
        // XXX: Non-None image types unimplemented.
        Image::ImageSet(..) |
        Image::Gradient(..) |
        Image::Image(..) |
        Image::CrossFade(..) |
        Image::PaintWorklet(..) |
        Image::None => None,
        Image::LightDark(..) => unreachable!("light-dark() should be disabled"),
    };

    let content = match &marker_info.style.get_counters().content {
        Content::Items(_) => generate_pseudo_element_content(&marker_info, context),
        Content::None => return None,
        Content::Normal => marker_image().or_else(|| {
            let counter_value = context
                .counter_snapshots
                .lock()
                .get(info.node)
                .map_or(0, |snapshot| {
                    snapshot.value(&stylo_atoms::Atom::from("list-item"))
                });
            Some(vec![PseudoElementContentItem::Text(marker_string(
                context,
                info.node,
                &list_style.list_style_type,
                counter_value,
            )?)])
        })?,
    };

    Some((marker_info, content))
}

fn symbol_to_string(symbol: &Symbol) -> &str {
    match symbol {
        Symbol::String(string) => string,
        Symbol::Ident(ident) => &ident.0,
    }
}

fn counter_range_contains(ranges: &CounterRanges, value: i32) -> bool {
    ranges.0.iter().any(|range| {
        let lower = match &range.start {
            CounterBound::Integer(bound) => bound.resolve().is_some_and(|bound| value >= bound),
            CounterBound::Infinite => true,
        };
        let upper = match &range.end {
            CounterBound::Integer(bound) => bound.resolve().is_some_and(|bound| value <= bound),
            CounterBound::Infinite => true,
        };
        lower && upper
    })
}

fn grapheme_count(value: &str) -> usize {
    GraphemeClusterSegmenter::new()
        .segment_str(value)
        .count()
        .saturating_sub(1)
}

fn extended_algorithm_style(
    context: &LayoutContext,
    node: ServoLayoutNode<'_>,
    property: LonghandId,
    counter_style: &CounterStyle,
    depth: usize,
) -> CounterStyle {
    if depth > 8 {
        return CounterStyle::decimal();
    }
    let CounterStyle::Name(name) = counter_style else {
        return counter_style.clone();
    };
    let shadow_rule = node
        .as_element()
        .and_then(|element| {
            (!element.property_is_from_outer_tree(&context.style_context, property))
                .then(|| element.containing_shadow_style_data())
                .flatten()
        })
        .and_then(|data| data.counter_style_rule(&name.0).cloned());
    let rule_and_origin = shadow_rule.map(|rule| (rule, Origin::Author)).or_else(|| {
        context
            .style_context
            .stylist
            .iter_extra_data_origins_rev()
            .find_map(|(extra, origin)| {
                extra
                    .counter_styles
                    .get(&name.0)
                    .map(|rule| (rule.clone(), origin))
            })
    });
    let Some((rule, origin)) = rule_and_origin else {
        return counter_style.clone();
    };
    let guard = match origin {
        Origin::UserAgent | Origin::User => context.style_context.guards.ua_or_user,
        Origin::Author => context.style_context.guards.author,
    };
    let rule = rule.read_with(guard);
    if let System::Extends(base) = rule.resolved_system() {
        return extended_algorithm_style(
            context,
            node,
            property,
            &CounterStyle::Name(base.clone()),
            depth + 1,
        );
    }
    counter_style.clone()
}

/// <https://drafts.csswg.org/css-counter-styles-3/#generate-a-counter>
pub(crate) fn generate_counter_representation(
    context: &LayoutContext,
    node: ServoLayoutNode<'_>,
    property: LonghandId,
    counter_style: &CounterStyle,
    value: i32,
) -> String {
    generate_counter_representation_with_depth(context, node, property, counter_style, value, 0)
}

fn generate_counter_representation_with_depth(
    context: &LayoutContext,
    node: ServoLayoutNode<'_>,
    property: LonghandId,
    counter_style: &CounterStyle,
    value: i32,
    depth: usize,
) -> String {
    if depth > 8 {
        return value.to_string();
    }
    if let CounterStyle::Name(name) = counter_style {
        let shadow_rule = node
            .as_element()
            .and_then(|element| {
                (!element.property_is_from_outer_tree(&context.style_context, property))
                    .then(|| element.containing_shadow_style_data())
                    .flatten()
            })
            .and_then(|data| data.counter_style_rule(&name.0).cloned());
        let rule_and_origin = shadow_rule.map(|rule| (rule, Origin::Author)).or_else(|| {
            context
                .style_context
                .stylist
                .iter_extra_data_origins_rev()
                .find_map(|(extra, origin)| {
                    extra
                        .counter_styles
                        .get(&name.0)
                        .map(|rule| (rule.clone(), origin))
                })
        });
        if let Some((rule, origin)) = rule_and_origin {
            let guard = match origin {
                Origin::UserAgent | Origin::User => context.style_context.guards.ua_or_user,
                Origin::Author => context.style_context.guards.author,
            };
            let rule = rule.read_with(guard);
            let descriptors = rule.descriptors();
            let system = rule.resolved_system();
            let in_range = descriptors
                .range
                .as_ref()
                .is_none_or(|ranges| ranges.0.is_empty() || counter_range_contains(ranges, value));
            let in_automatic_range = match system {
                System::Alphabetic | System::Symbolic => value > 0,
                System::Additive => value >= 0,
                _ => true,
            };
            if !in_range
                || (descriptors.range.as_ref().is_none_or(|r| r.0.is_empty())
                    && !in_automatic_range)
            {
                return descriptors.fallback.as_ref().map_or_else(
                    || value.to_string(),
                    |fallback| {
                        generate_counter_representation_with_depth(
                            context,
                            node,
                            property,
                            &CounterStyle::Name(fallback.0.clone()),
                            value,
                            depth + 1,
                        )
                    },
                );
            }

            let uses_negative = !matches!(system, System::Cyclic | System::Fixed { .. });
            let absolute_value = if value < 0 && uses_negative {
                value.checked_abs().unwrap_or(value)
            } else {
                value
            };
            let (representation, add_default_negative) = match system {
                System::Extends(base) => {
                    let base_style = CounterStyle::Name(base.clone());
                    let algorithm_style =
                        extended_algorithm_style(context, node, property, &base_style, depth + 1);
                    let has_override = descriptors.negative.is_some()
                        || descriptors.pad.is_some()
                        || descriptors
                            .range
                            .as_ref()
                            .is_some_and(|ranges| !ranges.0.is_empty());
                    let representation = match &algorithm_style {
                        CounterStyle::Name(name)
                            if has_override && name.0 == atom!("upper-roman") =>
                        {
                            (absolute_value > 0).then(|| roman_unbounded(absolute_value, true))
                        },
                        CounterStyle::Name(name)
                            if has_override && name.0 == atom!("lower-roman") =>
                        {
                            (absolute_value > 0).then(|| roman_unbounded(absolute_value, false))
                        },
                        _ => Some(generate_counter_representation_with_depth(
                            context,
                            node,
                            property,
                            &base_style,
                            if descriptors.negative.is_some() || descriptors.pad.is_some() {
                                absolute_value
                            } else {
                                value
                            },
                            depth + 1,
                        )),
                    };
                    let add_default_negative = matches!(&algorithm_style,
                        CounterStyle::Name(name) if has_override &&
                            matches!(name.0, atom!("upper-roman") | atom!("lower-roman")))
                        || descriptors.pad.is_some();
                    (representation, add_default_negative)
                },
                System::Additive => (
                    descriptors
                        .additive_symbols
                        .as_ref()
                        .and_then(|tuples| additive_symbols(absolute_value, &tuples.0)),
                    true,
                ),
                _ => (
                    descriptors.symbols.as_ref().and_then(|symbols| {
                        let symbols = &symbols.0;
                        match system {
                            System::Numeric => {
                                Some(positional_symbols(absolute_value, symbols, true))
                            },
                            System::Alphabetic => {
                                Some(positional_symbols(absolute_value, symbols, false))
                            },
                            System::Cyclic => {
                                let index = (i64::from(value) - 1).rem_euclid(symbols.len() as i64)
                                    as usize;
                                Some(symbol_to_string(&symbols[index]).to_string())
                            },
                            System::Symbolic if absolute_value > 0 => {
                                let index = (absolute_value as usize - 1) % symbols.len();
                                Some(
                                    symbol_to_string(&symbols[index])
                                        .repeat((absolute_value as usize - 1) / symbols.len() + 1),
                                )
                            },
                            System::Fixed { first_symbol_value } => {
                                let first = first_symbol_value
                                    .as_ref()
                                    .and_then(|n| n.resolve())
                                    .unwrap_or(1);
                                let index = i64::from(value) - i64::from(first);
                                (index >= 0 && (index as usize) < symbols.len())
                                    .then(|| symbol_to_string(&symbols[index as usize]).to_string())
                            },
                            System::Symbolic | System::Additive | System::Extends(_) => None,
                        }
                    }),
                    true,
                ),
            };
            if let Some(mut representation) = representation {
                if let Some(pad) = &descriptors.pad {
                    let width = pad.0.resolve().unwrap_or(0).max(0) as usize;
                    let negative_width = if value < 0 && uses_negative {
                        descriptors.negative.as_ref().map_or(1, |negative| {
                            grapheme_count(symbol_to_string(&negative.0))
                                + negative
                                    .1
                                    .as_ref()
                                    .map_or(0, |end| grapheme_count(symbol_to_string(end)))
                        })
                    } else {
                        0
                    };
                    let padding =
                        width.saturating_sub(grapheme_count(&representation) + negative_width);
                    representation = format!(
                        "{}{}",
                        symbol_to_string(&pad.1).repeat(padding.min(10000)),
                        representation
                    );
                }
                if value < 0 && uses_negative {
                    if let Some(negative) = &descriptors.negative {
                        return format!(
                            "{}{}{}",
                            symbol_to_string(&negative.0),
                            representation,
                            negative.1.as_ref().map_or("", symbol_to_string),
                        );
                    }
                    if add_default_negative {
                        return format!("-{representation}");
                    }
                }
                return representation;
            }
            return descriptors.fallback.as_ref().map_or_else(
                || value.to_string(),
                |fallback| {
                    generate_counter_representation_with_depth(
                        context,
                        node,
                        property,
                        &CounterStyle::Name(fallback.0.clone()),
                        value,
                        depth + 1,
                    )
                },
            );
        }
    }
    match counter_style {
        CounterStyle::None => String::new(),
        CounterStyle::String(string) => string.to_string(),
        CounterStyle::Name(name) => match name.0 {
            atom!("disc") => "\u{2022}".into(),
            atom!("circle") => "\u{25E6}".into(),
            atom!("square") => "\u{25AA}".into(),
            atom!("disclosure-open") => "\u{25BE}".into(),
            // TODO: Use U+25C2 "◂" depending on the direction.
            atom!("disclosure-closed") => "\u{25B8}".into(),
            atom!("decimal-leading-zero") if (0..10).contains(&value) => format!("0{value}"),
            atom!("decimal-leading-zero") if (-9..0).contains(&value) => format!("-0{}", -value),
            atom!("lower-alpha") | atom!("lower-latin") => {
                alphabetic(value, "abcdefghijklmnopqrstuvwxyz")
            },
            atom!("upper-alpha") | atom!("upper-latin") => {
                alphabetic(value, "ABCDEFGHIJKLMNOPQRSTUVWXYZ")
            },
            atom!("lower-greek") => alphabetic(value, "αβγδεζηθικλμνξοπρστυφχψω"),
            atom!("hiragana") => alphabetic(
                value,
                "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわゐゑをん",
            ),
            atom!("hiragana-iroha") => alphabetic(
                value,
                "いろはにほへとちりぬるをわかよたれそつねならむうゐのおくやまけふこえてあさきゆめみしゑひもせす",
            ),
            atom!("katakana") => alphabetic(
                value,
                "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヰヱヲン",
            ),
            atom!("katakana-iroha") => alphabetic(
                value,
                "イロハニホヘトチリヌルヲワカヨタレソツネナラムウヰノオクヤマケフコエテアサキユメミシヱヒモセス",
            ),
            atom!("lower-roman") => roman(value, false),
            atom!("upper-roman") => roman(value, true),
            atom!("armenian") | atom!("upper-armenian") => armenian(value, false),
            atom!("lower-armenian") => armenian(value, true),
            atom!("georgian") => georgian(value),
            atom!("hebrew") => hebrew(value),
            atom!("arabic-indic") => decimal_digits(value, '\u{660}'),
            atom!("bengali") => decimal_digits(value, '\u{9E6}'),
            atom!("cambodian") | atom!("khmer") => decimal_digits(value, '\u{17E0}'),
            atom!("devanagari") => decimal_digits(value, '\u{966}'),
            atom!("gujarati") => decimal_digits(value, '\u{AE6}'),
            atom!("gurmukhi") => decimal_digits(value, '\u{A66}'),
            atom!("kannada") => decimal_digits(value, '\u{CE6}'),
            atom!("lao") => decimal_digits(value, '\u{ED0}'),
            atom!("malayalam") => decimal_digits(value, '\u{D66}'),
            atom!("mongolian") => decimal_digits(value, '\u{1810}'),
            atom!("myanmar") => decimal_digits(value, '\u{1040}'),
            atom!("oriya") => decimal_digits(value, '\u{B66}'),
            atom!("persian") => decimal_digits(value, '\u{6F0}'),
            atom!("tamil") => decimal_digits(value, '\u{BE6}'),
            atom!("telugu") => decimal_digits(value, '\u{C66}'),
            atom!("thai") => decimal_digits(value, '\u{E50}'),
            atom!("tibetan") => decimal_digits(value, '\u{F20}'),
            atom!("japanese-informal") => cjk_additive(
                value,
                "一二三四五六七八九",
                ["千", "百", "十"],
                "〇",
                "マイナス",
                true,
            ),
            atom!("japanese-formal") => cjk_additive(
                value,
                "壱弐参四伍六七八九",
                ["阡", "百", "拾"],
                "零",
                "マイナス",
                false,
            ),
            atom!("korean-hangul-formal") => cjk_additive(
                value,
                "일이삼사오육칠팔구",
                ["천", "백", "십"],
                "영",
                "마이너스 ",
                false,
            ),
            atom!("korean-hanja-informal") => cjk_additive(
                value,
                "一二三四五六七八九",
                ["千", "百", "十"],
                "零",
                "마이너스 ",
                true,
            ),
            atom!("korean-hanja-formal") => cjk_additive(
                value,
                "壹貳參四五六七八九",
                ["仟", "百", "拾"],
                "零",
                "마이너스 ",
                false,
            ),
            atom!("cjk-decimal") if value >= 0 => decimal_symbols(value, "〇一二三四五六七八九"),
            atom!("cjk-earthly-branch") => {
                fixed_symbols(value, "子丑寅卯辰巳午未申酉戌亥", "〇一二三四五六七八九")
            },
            atom!("cjk-heavenly-stem") => {
                fixed_symbols(value, "甲乙丙丁戊己庚辛壬癸", "〇一二三四五六七八九")
            },
            _ if &*name.0 == "japanese" => cjk_additive(
                value,
                "一二三四五六七八九",
                ["千", "百", "十"],
                "〇",
                "マイナス",
                true,
            ),
            _ if &*name.0 == "korean-hangul" => cjk_additive(
                value,
                "일이삼사오육칠팔구",
                ["천", "백", "십"],
                "영",
                "마이너스 ",
                false,
            ),
            _ if &*name.0 == "korean-hanja" => cjk_additive(
                value,
                "一二三四五六七八九",
                ["千", "百", "十"],
                "零",
                "마이너스 ",
                true,
            ),
            _ => chinese_style(value, &name.0).unwrap_or_else(|| value.to_string()),
        },
        CounterStyle::Symbols { ty, symbols } => match ty {
            SymbolsType::Numeric => positional_symbols(value, &symbols.0, true),
            SymbolsType::Alphabetic => positional_symbols(value, &symbols.0, false),
            SymbolsType::Cyclic => {
                let index = (i64::from(value) - 1).rem_euclid(symbols.0.len() as i64) as usize;
                symbol_to_string(&symbols.0[index]).to_string()
            },
            SymbolsType::Symbolic if value > 0 => {
                let index = (value as usize - 1) % symbols.0.len();
                symbol_to_string(&symbols.0[index])
                    .repeat((value as usize - 1) / symbols.0.len() + 1)
            },
            SymbolsType::Fixed if value > 0 && (value as usize) <= symbols.0.len() => {
                symbol_to_string(&symbols.0[value as usize - 1]).to_string()
            },
            SymbolsType::Symbolic | SymbolsType::Fixed => value.to_string(),
        },
    }
}

fn decimal_digits(value: i32, zero: char) -> String {
    value
        .to_string()
        .chars()
        .map(|ch| {
            ch.to_digit(10)
                .and_then(|digit| char::from_u32(zero as u32 + digit))
                .unwrap_or(ch)
        })
        .collect()
}

fn decimal_symbols(value: i32, symbols: &str) -> String {
    let symbols: Vec<_> = symbols.chars().collect();
    value
        .to_string()
        .chars()
        .map(|ch| {
            ch.to_digit(10)
                .map(|digit| symbols[digit as usize])
                .unwrap_or(ch)
        })
        .collect()
}

fn fixed_symbols(value: i32, symbols: &str, fallback_digits: &str) -> String {
    if value < 0 {
        return value.to_string();
    }
    if value > 0 {
        if let Some(symbol) = symbols.chars().nth(value as usize - 1) {
            return symbol.to_string();
        }
    }
    decimal_symbols(value, fallback_digits)
}

fn additive_symbols(value: i32, tuples: &[AdditiveTuple]) -> Option<String> {
    let mut remaining = i64::from(value).abs();
    if remaining == 0 {
        return tuples
            .iter()
            .find(|tuple| tuple.weight.resolve() == Some(0))
            .map(|tuple| symbol_to_string(&tuple.symbol).to_string());
    }

    let mut result = String::new();
    for tuple in tuples {
        let Some(weight) = tuple.weight.resolve().filter(|weight| *weight > 0) else {
            continue;
        };
        let repetitions = remaining / i64::from(weight);
        if repetitions == 0 {
            continue;
        }
        let symbol = symbol_to_string(&tuple.symbol);
        if result.chars().count() as i64 + repetitions * symbol.chars().count() as i64 > 60 {
            return None;
        }
        for _ in 0..repetitions {
            result.push_str(symbol);
        }
        remaining -= repetitions * i64::from(weight);
        if remaining == 0 {
            return Some(if value < 0 {
                format!("-{result}")
            } else {
                result
            });
        }
    }
    None
}

fn armenian(value: i32, lowercase: bool) -> String {
    if !(1..=9999).contains(&value) {
        return value.to_string();
    }
    let mut result = String::new();
    for (place, first_symbol) in [(1000, 0x54c), (100, 0x543), (10, 0x53a), (1, 0x531)] {
        let digit = (value / place) % 10;
        if digit != 0 {
            let offset = if lowercase { 0x30 } else { 0 };
            result.push(char::from_u32(first_symbol + digit as u32 - 1 + offset).unwrap());
        }
    }
    result
}

fn georgian(value: i32) -> String {
    if !(1..=19999).contains(&value) {
        return value.to_string();
    }
    let mut result = String::new();
    if value >= 10000 {
        result.push('ჵ');
    }
    for (place, symbols) in [
        (1000, "ჩცძწჭხჴჯჰ"),
        (100, "რსტჳფქღყშ"),
        (10, "იკლმნჲოპჟ"),
        (1, "აბგდევზჱთ"),
    ] {
        let digit = (value / place) % 10;
        if digit != 0 {
            result.push(symbols.chars().nth(digit as usize - 1).unwrap());
        }
    }
    result
}

fn hebrew(value: i32) -> String {
    if !(1..=10999).contains(&value) {
        return value.to_string();
    }
    let mut result = String::new();
    let thousands = value / 1000;
    if thousands > 0 {
        result.push(char::from_u32(0x5d0 + thousands as u32 - 1).unwrap());
        result.push('׳');
    }
    let mut remaining = value % 1000;
    for (weight, symbol) in [
        (400, "ת"),
        (300, "ש"),
        (200, "ר"),
        (100, "ק"),
        (90, "צ"),
        (80, "פ"),
        (70, "ע"),
        (60, "ס"),
        (50, "נ"),
        (40, "מ"),
        (30, "ל"),
        (20, "כ"),
        (19, "יט"),
        (18, "יח"),
        (17, "יז"),
        (16, "טז"),
        (15, "טו"),
        (10, "י"),
        (9, "ט"),
        (8, "ח"),
        (7, "ז"),
        (6, "ו"),
        (5, "ה"),
        (4, "ד"),
        (3, "ג"),
        (2, "ב"),
        (1, "א"),
    ] {
        let repetitions = remaining / weight;
        for _ in 0..repetitions {
            result.push_str(symbol);
        }
        remaining -= repetitions * weight;
        if remaining == 0 {
            break;
        }
    }
    result
}

fn cjk_additive(
    value: i32,
    digits: &str,
    units: [&str; 3],
    zero: &str,
    negative: &str,
    omit_one: bool,
) -> String {
    if !(-9999..=9999).contains(&value) {
        return if value >= 0 {
            decimal_symbols(value, "〇一二三四五六七八九")
        } else {
            value.to_string()
        };
    }
    if value == 0 {
        return zero.to_string();
    }
    let magnitude = value.abs();
    let mut result = if value < 0 {
        negative.to_string()
    } else {
        String::new()
    };
    for (place, unit) in [(1000, units[0]), (100, units[1]), (10, units[2]), (1, "")] {
        let digit = (magnitude / place) % 10;
        if digit == 0 {
            continue;
        }
        if !omit_one || digit != 1 || place == 1 {
            result.push(digits.chars().nth(digit as usize - 1).unwrap());
        }
        result.push_str(unit);
    }
    result
}

fn chinese_style(value: i32, name: &str) -> Option<String> {
    let (digits, units, negative, informal) = match name {
        "simp-chinese" | "simp-chinese-informal" => {
            ("一二三四五六七八九", ["千", "百", "十"], "负", true)
        },
        "simp-chinese-formal" => ("壹贰叁肆伍陆柒捌玖", ["仟", "佰", "拾"], "负", false),
        "trad-chinese" | "trad-chinese-informal" | "cjk-ideographic" => {
            ("一二三四五六七八九", ["千", "百", "十"], "負", true)
        },
        "trad-chinese-formal" => ("壹貳參肆伍陸柒捌玖", ["仟", "佰", "拾"], "負", false),
        _ => return None,
    };
    if !(-9999..=9999).contains(&value) {
        return Some(if value >= 0 {
            decimal_symbols(value, "〇一二三四五六七八九")
        } else {
            value.to_string()
        });
    }
    if value == 0 {
        return Some("零".into());
    }
    let magnitude = value.abs();
    let mut result = if value < 0 {
        negative.to_string()
    } else {
        String::new()
    };
    let mut pending_zero = false;
    let mut seen_digit = false;
    for (place, unit) in [(1000, units[0]), (100, units[1]), (10, units[2]), (1, "")] {
        let digit = (magnitude / place) % 10;
        if digit == 0 {
            if seen_digit {
                pending_zero = true;
            }
            continue;
        }
        if pending_zero {
            result.push('零');
            pending_zero = false;
        }
        if !informal || !(10..=19).contains(&magnitude) || place != 10 {
            result.push(digits.chars().nth(digit as usize - 1).unwrap());
        }
        result.push_str(unit);
        seen_digit = true;
    }
    Some(result)
}

fn alphabetic(value: i32, symbols: &str) -> String {
    if value <= 0 {
        return value.to_string();
    }
    let symbols: Vec<char> = symbols.chars().collect();
    let mut value = value as usize;
    let mut result = Vec::new();
    while value > 0 {
        value -= 1;
        result.push(symbols[value % symbols.len()]);
        value /= symbols.len();
    }
    result.into_iter().rev().collect()
}

fn roman(value: i32, uppercase: bool) -> String {
    if !(1..=3999).contains(&value) {
        return value.to_string();
    }
    roman_unbounded(value, uppercase)
}

fn roman_unbounded(value: i32, uppercase: bool) -> String {
    let mut value = value;
    let mut result = String::new();
    for (number, symbol) in [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ] {
        while value >= number {
            result.push_str(symbol);
            value -= number;
        }
    }
    if uppercase {
        result
    } else {
        result.to_ascii_lowercase()
    }
}

fn positional_symbols(value: i32, symbols: &[Symbol], zero_based: bool) -> String {
    if symbols.is_empty() || (!zero_based && value <= 0) {
        return value.to_string();
    }
    let mut number = i64::from(value).unsigned_abs();
    let base = symbols.len() as u64;
    let mut parts = Vec::new();
    loop {
        if !zero_based {
            number -= 1;
        }
        parts.push(symbol_to_string(&symbols[(number % base) as usize]));
        number /= base;
        if number == 0 {
            break;
        }
    }
    let mut result = if value < 0 {
        String::from("-")
    } else {
        String::new()
    };
    for part in parts.into_iter().rev() {
        result.push_str(part);
    }
    result
}

/// <https://drafts.csswg.org/css-lists/#marker-string>
pub(crate) fn marker_string(
    context: &LayoutContext,
    node: ServoLayoutNode<'_>,
    list_style_type: &ListStyleType,
    value: i32,
) -> Option<String> {
    let mut suffix = match &list_style_type.0 {
        CounterStyle::None => return None,
        CounterStyle::String(string) => return Some(string.to_string()),
        CounterStyle::Name(name) => match name.0 {
            atom!("disc") |
            atom!("circle") |
            atom!("square") |
            atom!("disclosure-open") |
            atom!("disclosure-closed") => " ",
            atom!("hiragana") |
            atom!("hiragana-iroha") |
            atom!("katakana") |
            atom!("katakana-iroha") |
            atom!("cjk-decimal") |
            atom!("cjk-earthly-branch") |
            atom!("cjk-heavenly-stem") |
            atom!("japanese-informal") |
            atom!("japanese-formal") |
            atom!("simp-chinese-informal") |
            atom!("simp-chinese-formal") |
            atom!("trad-chinese-informal") |
            atom!("trad-chinese-formal") |
            atom!("cjk-ideographic") => "\u{3001}", /* "、" */
            atom!("korean-hangul-formal") |
            atom!("korean-hanja-informal") |
            atom!("korean-hanja-formal") => ", ",
            atom!("ethiopic-numeric") => "/ ",
            _ if &*name.0 == "japanese" => "\u{3001}",
            _ if matches!(
                &*name.0,
                "simp-chinese" | "simp-chinese-formal" | "trad-chinese" | "trad-chinese-formal"
            ) =>
            {
                "\u{3001}"
            },
            _ if matches!(&*name.0, "korean-hangul" | "korean-hanja") => ", ",
            _ => ". ",
        },
        CounterStyle::Symbols { .. } => " ",
    }
    .to_string();
    let mut prefix = String::new();
    if let CounterStyle::Name(name) = &list_style_type.0 {
        let shadow_rule = node
            .as_element()
            .and_then(|element| {
                (!element
                    .property_is_from_outer_tree(&context.style_context, LonghandId::ListStyleType))
                .then(|| element.containing_shadow_style_data())
                .flatten()
            })
            .and_then(|data| data.counter_style_rule(&name.0).cloned());
        let rule_and_origin = shadow_rule.map(|rule| (rule, Origin::Author)).or_else(|| {
            context
                .style_context
                .stylist
                .iter_extra_data_origins_rev()
                .find_map(|(extra, origin)| {
                    extra
                        .counter_styles
                        .get(&name.0)
                        .map(|rule| (rule.clone(), origin))
                })
        });
        if let Some((rule, origin)) = rule_and_origin {
            let guard = match origin {
                Origin::UserAgent | Origin::User => context.style_context.guards.ua_or_user,
                Origin::Author => context.style_context.guards.author,
            };
            let rule = rule.read_with(guard);
            if let Some(symbol) = &rule.descriptors().prefix {
                prefix = symbol_to_string(symbol).to_string();
            }
            suffix = rule.descriptors().suffix.as_ref().map_or_else(
                || ". ".to_string(),
                |symbol| symbol_to_string(symbol).to_string(),
            );
        }
    }
    Some(format!(
        "{}{}{}",
        prefix,
        generate_counter_representation(
            context,
            node,
            LonghandId::ListStyleType,
            &list_style_type.0,
            value,
        ),
        suffix
    ))
}
