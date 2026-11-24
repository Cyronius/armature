/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Resolution of the `font` shorthand in workers, which have no layout to resolve it with.
//!
//! The value is parsed and cascaded with a [`Stylist`] of their own, against the default font of
//! the canvas, which is what an offscreen canvas resolves its font against.

use std::cell::RefCell;
use std::sync::Arc;

use euclid::{Scale, Size2D};
use fonts::{FontContext, FontGroup};
use servo_arc::Arc as ServoArc;
use servo_config::pref;
use servo_url::ServoUrl;
use style::Zero;
use style::context::QuirksMode;
use style::device::Device;
use style::device::servo::{FontMetricsProvider, MediaData};
use style::font_metrics::FontMetrics;
use style::media_queries::MediaType;
use style::properties::style_structs::Font;
use style::properties::{
    ComputedValues, Importance, PropertyDeclarationBlock, PropertyId, ShorthandId,
    SourcePropertyDeclaration, parse_one_declaration_into,
};
use style::queries::values::PrefersColorScheme;
use style::servo::media_features::PointerCapabilities;
use style::shared_lock::{SharedRwLock, StylesheetGuards};
use style::stylesheets::{CssRuleType, Origin, UrlExtraData};
use style::stylist::Stylist;
use style::values::computed::font::GenericFontFamily;
use style::values::computed::{CSSPixelLength, FontSize, Length, NonNegativeLength};
use style::values::specified::font::{KeywordInfo, QueryFontMetricsFlags};
use style_traits::{CSSPixel, ParsingMode};

use crate::layout_dom::ServoDangerousStyleElement;

struct WorkerFontMetricsProvider(Arc<FontContext>);

impl WorkerFontMetricsProvider {
    fn font_group(&self, font: &Font, base_size: CSSPixelLength) -> Arc<FontGroup> {
        self.0
            .font_group_with_size(ServoArc::new(font.clone()), base_size.into())
    }
}

impl std::fmt::Debug for WorkerFontMetricsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("WorkerFontMetricsProvider").finish()
    }
}

impl FontMetricsProvider for WorkerFontMetricsProvider {
    fn normal_line_height(&self, font: &Font, base_size: CSSPixelLength) -> CSSPixelLength {
        self.font_group(font, base_size)
            .first(&self.0)
            .map(|font| CSSPixelLength::from(font.metrics().line_gap))
            .unwrap_or(base_size * 1.2)
    }

    fn query_font_metrics(
        &self,
        _vertical: bool,
        font: &Font,
        base_size: CSSPixelLength,
        _flags: QueryFontMetricsFlags,
    ) -> FontMetrics {
        let Some(metrics) = self
            .font_group(font, base_size)
            .first(&self.0)
            .map(|font| font.metrics().clone())
        else {
            return Default::default();
        };
        FontMetrics {
            x_height: Some(metrics.x_height)
                .filter(|x_height| !x_height.is_zero())
                .map(CSSPixelLength::from),
            zero_advance_measure: metrics.zero_horizontal_advance.map(CSSPixelLength::from),
            cap_height: None,
            ic_width: metrics.ic_horizontal_advance.map(CSSPixelLength::from),
            ascent: metrics.ascent.into(),
            script_percent_scale_down: None,
            script_script_percent_scale_down: None,
        }
    }

    fn base_size_for_generic(&self, generic: GenericFontFamily) -> Length {
        Length::new(match generic {
            GenericFontFamily::Monospace => pref!(fonts_default_monospace_size),
            _ => pref!(fonts_default_size),
        } as f32)
        .max(Length::new(0.0))
    }
}

struct WorkerFontResolver {
    stylist: Stylist,
    lock: SharedRwLock,
}

impl WorkerFontResolver {
    fn new(font_context: Arc<FontContext>) -> Self {
        let mut font = Font::initial_values();
        let default_font_size = pref!(fonts_default_size);
        font.font_size = FontSize {
            computed_size: NonNegativeLength::new(default_font_size as f32),
            used_size: NonNegativeLength::new(default_font_size as f32),
            keyword_info: KeywordInfo::medium(),
        };
        let device = Device::new(
            ComputedValues::initial_values_with_font_override(font),
            Box::new(WorkerFontMetricsProvider(font_context)),
            MediaData {
                media_type: MediaType::screen(),
                quirks_mode: QuirksMode::NoQuirks,
                viewport_size: Size2D::<f32, CSSPixel>::new(800.0, 600.0),
                device_size: Size2D::new(800.0, 600.0),
                device_pixel_ratio: Scale::new(1.0),
                prefers_color_scheme: PrefersColorScheme::Light,
                primary_pointer_capabilities: PointerCapabilities::default(),
                all_pointer_capabilities: PointerCapabilities::default(),
            },
        );
        Self {
            stylist: Stylist::new(device, QuirksMode::NoQuirks),
            lock: SharedRwLock::new(),
        }
    }

    fn declarations(value: &str, url: &ServoUrl) -> Option<PropertyDeclarationBlock> {
        let mut declarations = SourcePropertyDeclaration::default();
        parse_one_declaration_into(
            &mut declarations,
            PropertyId::NonCustom(ShorthandId::Font.into()),
            value,
            Origin::Author,
            &UrlExtraData(url.get_arc()),
            None,
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            CssRuleType::Style,
        )
        .ok()?;
        // CSS-wide keywords and values with arbitrary substitution functions, such as var(),
        // are not valid canvas fonts.
        // <https://html.spec.whatwg.org/multipage/#dom-context-2d-font>
        if declarations
            .declarations
            .iter()
            .any(|declaration| !declaration.may_serialize_as_part_of_shorthand())
        {
            return None;
        }
        let mut block = PropertyDeclarationBlock::new();
        block.extend(declarations.drain(), Importance::Normal);
        Some(block)
    }

    fn compute(
        &self,
        parent_style: Option<&ComputedValues>,
        declarations: PropertyDeclarationBlock,
    ) -> ServoArc<ComputedValues> {
        let parent_style = parent_style.unwrap_or_else(|| self.stylist.device().default_computed_values());
        let guard = self.lock.read();
        self.stylist
            .compute_for_declarations::<ServoDangerousStyleElement<'static>>(
                &StylesheetGuards::same(&guard),
                parent_style,
                ServoArc::new(self.lock.wrap(declarations)),
            )
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-context-2d-font>
    fn resolve(&self, value: &str, url: &ServoUrl) -> Option<ServoArc<Font>> {
        // Step 1. Parse the given font property value.
        let declarations = Self::declarations(value, url)?;
        // Step 2. The font of a detached element is the default, `10px sans-serif`.
        let default_declarations = Self::declarations("10px sans-serif", url)?;
        let parent_style = self.compute(None, default_declarations);
        // Step 3. Resolve the parsed value against that.
        Some(self.compute(Some(&parent_style), declarations).clone_font())
    }
}

thread_local! {
    static RESOLVER: RefCell<Option<WorkerFontResolver>> = const { RefCell::new(None) };
}

/// Resolves the value of the `font` shorthand against the default font, without layout.
pub(crate) fn resolve_font_without_layout(
    font_context: &Arc<FontContext>,
    value: &str,
    url: &ServoUrl,
) -> Option<ServoArc<Font>> {
    RESOLVER.with_borrow_mut(|resolver| {
        resolver
            .get_or_insert_with(|| WorkerFontResolver::new(font_context.clone()))
            .resolve(value, url)
    })
}
