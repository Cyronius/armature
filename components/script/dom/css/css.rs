/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use cssparser::{Parser, serialize_identifier};
use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::codegen::GenericBindings::CSSBinding::PropertyDefinition;
use script_bindings::num::Finite;
use script_bindings::reflector::Reflector;
use style::stylesheets::supports_rule::{Declaration, parse_condition_or_declaration};
use style::stylesheets::{CssRuleType, UrlExtraData};
use style::stylist::RegisterCustomPropertyResult;
use style_traits::ParsingMode;

use crate::css::css::parser_context_for_anonymous_content;
use crate::dom::bindings::codegen::Bindings::CSSBinding::CSSMethods;
use crate::dom::bindings::codegen::Bindings::WindowBinding::Window_Binding::WindowMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::css::cssunitvalue::CSSUnitValue;
use crate::dom::window::Window;
use crate::dom::worklet::Worklet;

#[dom_struct]
#[expect(clippy::upper_case_acronyms)]
pub(crate) struct CSS {
    reflector_: Reflector,
}

macro_rules! numeric_factory {
    ($method:ident, $unit:literal) => {
        fn $method(cx: &mut JSContext, win: &Window, value: Finite<f64>) -> DomRoot<CSSUnitValue> {
            CSSUnitValue::new(cx, win.as_global_scope(), *value, $unit)
        }
    };
}

impl CSSMethods<crate::DomTypeHolder> for CSS {
    numeric_factory!(Number, "number");
    numeric_factory!(Percent, "percent");
    numeric_factory!(Em, "em");
    numeric_factory!(Ex, "ex");
    numeric_factory!(Ch, "ch");
    numeric_factory!(Ic, "ic");
    numeric_factory!(Rem, "rem");
    numeric_factory!(Lh, "lh");
    numeric_factory!(Rlh, "rlh");
    numeric_factory!(Vw, "vw");
    numeric_factory!(Vh, "vh");
    numeric_factory!(Vi, "vi");
    numeric_factory!(Vb, "vb");
    numeric_factory!(Vmin, "vmin");
    numeric_factory!(Vmax, "vmax");
    numeric_factory!(Cm, "cm");
    numeric_factory!(Mm, "mm");
    numeric_factory!(Q, "q");
    numeric_factory!(In, "in");
    numeric_factory!(Pt, "pt");
    numeric_factory!(Pc, "pc");
    numeric_factory!(Px, "px");
    numeric_factory!(Deg, "deg");
    numeric_factory!(Grad, "grad");
    numeric_factory!(Rad, "rad");
    numeric_factory!(Turn, "turn");
    numeric_factory!(S, "s");
    numeric_factory!(Ms, "ms");
    numeric_factory!(Hz, "hz");
    numeric_factory!(KHz, "khz");
    numeric_factory!(Dpi, "dpi");
    numeric_factory!(Dpcm, "dpcm");
    numeric_factory!(Dppx, "dppx");
    numeric_factory!(Fr, "fr");
    /// <https://drafts.csswg.org/cssom/#the-css.escape()-method>
    fn Escape(_: &Window, ident: DOMString) -> Fallible<DOMString> {
        let mut escaped = String::new();
        serialize_identifier(&ident.str(), &mut escaped).unwrap();
        Ok(DOMString::from(escaped))
    }

    /// <https://drafts.csswg.org/css-conditional/#dom-css-supports>
    fn Supports(win: &Window, property: DOMString, value: DOMString) -> bool {
        let mut decl = String::new();
        serialize_identifier(&property.str(), &mut decl).unwrap();
        decl.push_str(": ");
        decl.push_str(&value.str());
        let decl = Declaration(decl);
        let url_data = UrlExtraData(win.Document().url().get_arc());
        let mut context = parser_context_for_anonymous_content(
            CssRuleType::Style,
            ParsingMode::DEFAULT,
            &url_data,
        );
        decl.eval(&mut context)
    }

    /// <https://drafts.csswg.org/css-conditional/#dom-css-supports>
    fn Supports_(win: &Window, condition: DOMString) -> bool {
        let condition = condition.str();
        let mut input = Parser::new(&condition);
        let cond = match parse_condition_or_declaration(&mut input) {
            Ok(c) => c,
            Err(..) => return false,
        };

        let url_data = UrlExtraData(win.Document().url().get_arc());
        let mut context = parser_context_for_anonymous_content(
            CssRuleType::Style,
            ParsingMode::DEFAULT,
            &url_data,
        );
        cond.eval(&mut context)
    }

    /// <https://drafts.css-houdini.org/css-paint-api-1/#paint-worklet>
    fn PaintWorklet(cx: &mut JSContext, win: &Window) -> DomRoot<Worklet> {
        win.paint_worklet(cx)
    }

    /// <https://drafts.css-houdini.org/css-properties-values-api/#the-registerproperty-function>
    fn RegisterProperty(window: &Window, property_definition: &PropertyDefinition) -> Fallible<()> {
        use RegisterCustomPropertyResult::*;
        let result = window.layout_mut().stylist_mut().register_custom_property(
            &UrlExtraData(window.get_url().get_arc()),
            &property_definition.name.str(),
            &property_definition.syntax.str(),
            property_definition.inherits,
            property_definition
                .initialValue
                .as_ref()
                .map(|value| value.str())
                .as_deref(),
        );
        Err(match result {
            SuccessfullyRegistered => return Ok(()),
            InvalidName |
            InvalidSyntax |
            InvalidInitialValue |
            NoInitialValue |
            InitialValueNotComputationallyIndependent => Error::Syntax(Some(
                "CSS property value is not computationally independent".into(),
            )),
            AlreadyRegistered => {
                Error::InvalidModification(Some("CSS property is already registered".into()))
            },
        })
    }
}
