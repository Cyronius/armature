/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use layout_api::LayoutNode;
use script::layout_dom::ServoLayoutNode;
use style::counter_style::{AdditiveTuple, CounterStyle, Symbol, SymbolsType, System};
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
            if let System::Extends(base) = rule.resolved_system() {
                return generate_counter_representation_with_depth(
                    context,
                    node,
                    property,
                    &CounterStyle::Name(base.clone()),
                    value,
                    depth + 1,
                );
            }
            if let System::Additive = rule.resolved_system() {
                if let Some(tuples) = &rule.descriptors().additive_symbols {
                    if let Some(representation) = additive_symbols(value, &tuples.0) {
                        return representation;
                    }
                }
            }
            if let Some(symbols) = &rule.descriptors().symbols {
                let symbols = &symbols.0;
                let representation = match rule.resolved_system() {
                    System::Numeric => Some(positional_symbols(value, symbols, true)),
                    System::Alphabetic => Some(positional_symbols(value, symbols, false)),
                    System::Cyclic => {
                        let index =
                            (i64::from(value) - 1).rem_euclid(symbols.len() as i64) as usize;
                        Some(symbol_to_string(&symbols[index]).to_string())
                    },
                    System::Symbolic if value > 0 => {
                        let index = (value as usize - 1) % symbols.len();
                        Some(
                            symbol_to_string(&symbols[index])
                                .repeat((value as usize - 1) / symbols.len() + 1),
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
                };
                if let Some(representation) = representation {
                    return representation;
                }
            }
            if let Some(fallback) = &rule.descriptors().fallback {
                return generate_counter_representation_with_depth(
                    context,
                    node,
                    property,
                    &CounterStyle::Name(fallback.0.clone()),
                    value,
                    depth + 1,
                );
            }
            return value.to_string();
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
            atom!("japanese-informal") if value == 0 => "\u{3007}".into(),
            atom!("korean-hangul-formal") if value == 0 => "\u{C601}".into(),
            atom!("korean-hanja-informal") |
            atom!("korean-hanja-formal") |
            atom!("japanese-formal") |
            atom!("simp-chinese-informal") |
            atom!("simp-chinese-formal") |
            atom!("trad-chinese-informal") |
            atom!("trad-chinese-formal") |
            atom!("cjk-ideographic")
                if value == 0 =>
            {
                "\u{96F6}".into()
            },
            atom!("cjk-decimal") if value >= 0 => decimal_symbols(value, "〇一二三四五六七八九"),
            atom!("cjk-earthly-branch") => {
                fixed_symbols(value, "子丑寅卯辰巳午未申酉戌亥", "〇一二三四五六七八九")
            },
            atom!("cjk-heavenly-stem") => {
                fixed_symbols(value, "甲乙丙丁戊己庚辛壬癸", "〇一二三四五六七八九")
            },
            _ => value.to_string(),
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
    let suffix = match &list_style_type.0 {
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
            _ => ". ",
        },
        CounterStyle::Symbols { .. } => " ",
    };
    Some(format!(
        "{}{}",
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
