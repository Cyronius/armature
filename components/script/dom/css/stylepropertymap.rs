/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::reflect_dom_object;

use super::cssstyledeclaration::CSSStyleDeclaration;
use super::stylepropertymapreadonly::StylePropertyMapReadOnly;
use crate::dom::bindings::codegen::Bindings::CSSBinding::CSSMethods;
use crate::dom::bindings::codegen::Bindings::CSSStyleDeclarationBinding::CSSStyleDeclarationMethods;
use crate::dom::bindings::codegen::Bindings::CSSStyleValueBinding::CSSStyleValueMethods;
use crate::dom::bindings::codegen::Bindings::StylePropertyMapBinding::StylePropertyMapMethods;
use crate::dom::bindings::codegen::UnionTypes::CSSStyleValueOrString;
use crate::dom::bindings::error::{Error, ErrorResult};
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::css::css::CSS;
use crate::dom::window::Window;

#[dom_struct]
pub(crate) struct StylePropertyMap {
    style_property_map_read_only: StylePropertyMapReadOnly,
}

impl StylePropertyMap {
    pub(crate) fn new(
        cx: &mut JSContext,
        global: &Window,
        style: &CSSStyleDeclaration,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                style_property_map_read_only: StylePropertyMapReadOnly::for_style(style),
            }),
            global,
        )
    }

    fn style(&self) -> &CSSStyleDeclaration {
        self.style_property_map_read_only.style().unwrap()
    }

    fn serialize_values(
        property: &DOMString,
        values: Vec<CSSStyleValueOrString>,
    ) -> Result<DOMString, Error> {
        if values.is_empty() {
            return Err(Error::Type(c"A CSS value is required".to_owned()));
        }
        let values = values
            .into_iter()
            .map(|value| match value {
                CSSStyleValueOrString::CSSStyleValue(value) => {
                    if value
                        .associated_property()
                        .is_some_and(|associated| property.str() != associated)
                    {
                        return Err(Error::Type(c"Value belongs to another property".to_owned()));
                    }
                    Ok(value.Stringifier().to_string())
                },
                CSSStyleValueOrString::String(value) => Ok(value.to_string()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DOMString::from(values.join(", ")))
    }

    fn validate_value(&self, property: &DOMString, value: &DOMString) -> ErrorResult {
        if !property.str().starts_with("--") &&
            !CSS::Supports(&self.global(), property.clone(), value.clone())
        {
            return Err(Error::Type(c"Invalid CSS property value".to_owned()));
        }
        Ok(())
    }
}

impl StylePropertyMapMethods<crate::DomTypeHolder> for StylePropertyMap {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-stylepropertymap-set>
    fn Set(
        &self,
        cx: &mut JSContext,
        property: DOMString,
        values: Vec<CSSStyleValueOrString>,
    ) -> ErrorResult {
        let property = StylePropertyMapReadOnly::validated_property(property)?;
        let value = Self::serialize_values(&property, values)?;
        self.validate_value(&property, &value)?;
        self.style()
            .SetProperty(cx, property, value, DOMString::new())
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-stylepropertymap-append>
    fn Append(
        &self,
        cx: &mut JSContext,
        property: DOMString,
        values: Vec<CSSStyleValueOrString>,
    ) -> ErrorResult {
        let property = StylePropertyMapReadOnly::validated_property(property)?;
        if values.is_empty() {
            return Ok(());
        }
        let value = Self::serialize_values(&property, values)?;
        let previous = self.style().GetPropertyValue(property.clone());
        let value = if previous.is_empty() {
            value
        } else {
            DOMString::from(format!("{previous}, {value}"))
        };
        self.validate_value(&property, &value)?;
        self.style()
            .SetProperty(cx, property, value, DOMString::new())
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-stylepropertymap-delete>
    fn Delete(&self, cx: &mut JSContext, property: DOMString) -> ErrorResult {
        let property = StylePropertyMapReadOnly::validated_property(property)?;
        self.style().RemoveProperty(cx, property).map(|_| ())
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-stylepropertymap-clear>
    fn Clear(&self, cx: &mut JSContext) -> ErrorResult {
        self.style().SetCssText(cx, DOMString::new())
    }
}
