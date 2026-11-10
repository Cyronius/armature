/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::ffi::CString;
use std::ops::ControlFlow;
use std::ptr::NonNull;
use std::sync::LazyLock;

use cssparser::Parser;
use dom_struct::dom_struct;
use js::context::JSContext;
use js::conversions::{
    ConversionResult, FromJSValConvertible, ToJSValConvertible, jsstr_to_string,
};
use js::gc::{HandleValue, RootedVec};
use js::jsapi::{Heap, JS_GetPropertyById, JSITER_OWNONLY, JSObject};
use js::jsval::{ObjectValue, UndefinedValue};
use js::rust::{
    ForOfIterationFailure, HandleObject, IdVector, IntoHandle, IntoMutableHandle, for_of,
};
use rustc_hash::FxHashMap;
use script_bindings::cell::DomRefCell;
use script_bindings::codegen::GenericBindings::AnimationEffectBinding::{
    FillMode, OptionalEffectTiming, PlaybackDirection,
};
use script_bindings::codegen::GenericBindings::KeyframeEffectBinding::{
    BaseKeyframe, BasePropertyIndexedKeyframe, CompositeOperation, CompositeOperationOrAuto,
};
use script_bindings::codegen::GenericBindings::WindowBinding::WindowMethods;
use script_bindings::codegen::GenericUnionTypes::{
    CompositeOperationOrAutoOrCompositeOperationOrAutoSequence, DoubleOrDoubleOrNullSequence,
    StringOrStringSequence, UnrestrictedDoubleOrKeyframeEffectOptions, UnrestrictedDoubleOrString,
};
use script_bindings::conversions::StringificationBehavior;
use script_bindings::error::{Error, Fallible};
use script_bindings::inheritance::Castable;
use script_bindings::js::{
    GetPropertyKeys, JS_DefineProperty, JS_IdToValue, JS_NewObject, PropertyFlags,
};
use script_bindings::num::Finite;
use script_bindings::reflector::reflect_dom_object_with_proto;
use script_bindings::root::DomRoot;
use script_bindings::str::DOMString;
use servo_arc::Arc;
use style::animation::ScriptKeyframe;
use style::logical_geometry::WritingMode;
use style::parser::{Parse, ParserContext};
use style::properties::generated::PropertyDeclaration;
use style::properties::longhands::animation_composition::computed_value::single_value::T as AnimationComposition;
use style::properties::{
    Importance, NonCustomPropertyId, PropertyDeclarationBlock, PropertyDeclarationId,
    PropertyDeclarationIdSet, PropertyId, SourcePropertyDeclaration,
};
use style::selector_parser::PseudoElement;
use style::stylesheets::CssRuleType;
use style::values::computed::TimingFunction as ComputedTimingFunction;
use style::values::specified::TimingFunction as SpecifiedTimingFunction;
use style_traits::{CssWriter, ParsingMode, ToCss};

use crate::css::css::parser_context_for_document;
use crate::dom::Document;
use crate::dom::animationeffect::AnimationEffect;
use crate::dom::bindings::codegen::Bindings::KeyframeEffectBinding::{
    BaseComputedKeyframe, KeyframeEffectMethods,
};
use crate::dom::bindings::root::MutNullableDom;
use crate::dom::element::Element;
use crate::dom::window::Window;

/// <https://drafts.csswg.org/web-animations-1/#keyframeeffect>
#[dom_struct]
pub(crate) struct KeyframeEffect {
    animationeffect: AnimationEffect,

    /// <https://drafts.csswg.org/web-animations-1/#effect-target-target-element>
    target_element: MutNullableDom<Element>,

    /// <https://drafts.csswg.org/web-animations-1/#keyframe>
    keyframes: DomRefCell<Vec<Keyframe>>,

    /// <https://drafts.csswg.org/web-animations-1/#effect-composite-operation>
    composite: Cell<CompositeOperation>,

    /// <https://drafts.csswg.org/web-animations-1/#effect-target-target-pseudo-selector>
    ///
    target_pseudo_selector: DomRefCell<Option<DOMString>>,

    /// The keyframes in the form that style uses, built when first needed after the
    /// keyframes change.
    #[no_trace]
    #[ignore_malloc_size_of = "Arc"]
    keyframes_for_style: DomRefCell<Option<Arc<Vec<ScriptKeyframe>>>>,

    /// Whether this is the effect of a CSS animation or transition whose keyframes come
    /// from style, because script has not replaced them.
    css_keyframes: Cell<bool>,
}

impl KeyframeEffect {
    pub(crate) fn new_inherited(window: &Window) -> Self {
        Self {
            animationeffect: AnimationEffect::new_inherited(window),
            target_element: Default::default(),
            keyframes: Default::default(),
            composite: Cell::new(CompositeOperation::Replace),
            target_pseudo_selector: Default::default(),
            keyframes_for_style: Default::default(),
            css_keyframes: Cell::new(false),
        }
    }

    /// Create the effect of a CSS animation or transition, whose keyframes come from style.
    pub(crate) fn new_for_css(
        cx: &mut JSContext,
        window: &Window,
        target: &Element,
        pseudo_selector: Option<DOMString>,
    ) -> DomRoot<Self> {
        let effect = Self::new(cx, window);
        effect.target_element.set(Some(target));
        *effect.target_pseudo_selector.borrow_mut() = pseudo_selector;
        effect.css_keyframes.set(true);
        effect
    }

    /// Whether the keyframes of this effect come from a CSS animation or transition.
    pub(crate) fn uses_css_keyframes(&self) -> bool {
        self.css_keyframes.get()
    }

    /// <https://drafts.csswg.org/web-animations-1/#effect-target-target-pseudo-selector>
    pub(crate) fn target_pseudo_selector(&self) -> Option<DOMString> {
        self.target_pseudo_selector.borrow().clone()
    }

    fn new_with_proto(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
    ) -> DomRoot<Self> {
        reflect_dom_object_with_proto(cx, Box::new(Self::new_inherited(window)), window, proto)
    }

    pub(crate) fn new(cx: &mut JSContext, window: &Window) -> DomRoot<Self> {
        Self::new_with_proto(cx, window, None)
    }

    /// <https://drafts.csswg.org/web-animations-1/#effect-target-target-element>
    /// The originating element whose style or pseudo-element style this effect changes.
    pub(crate) fn target_for_style(&self) -> Option<DomRoot<Element>> {
        self.target_element.get()
    }

    pub(crate) fn target_pseudo_for_style(&self) -> Option<PseudoElement> {
        self.target_pseudo_selector
            .borrow()
            .as_ref()
            .and_then(|selector| pseudo_element_from_selector(&selector.str()))
    }

    /// The composite operation of this effect, in the form that style uses.
    pub(crate) fn composite_for_style(&self) -> AnimationComposition {
        match self.composite.get() {
            CompositeOperation::Replace => AnimationComposition::Replace,
            CompositeOperation::Add => AnimationComposition::Add,
            CompositeOperation::Accumulate => AnimationComposition::Accumulate,
        }
    }

    /// The keyframes of this effect in the form that style uses.
    pub(crate) fn keyframes_for_style(&self) -> Arc<Vec<ScriptKeyframe>> {
        let mut cached = self.keyframes_for_style.borrow_mut();
        if let Some(keyframes) = cached.as_ref() {
            return keyframes.clone();
        }
        let keyframes: Arc<Vec<ScriptKeyframe>> = Arc::new(
            self.keyframes
                .borrow()
                .iter()
                .map(|keyframe| {
                    let mut declarations = PropertyDeclarationBlock::new();
                    for declaration in &keyframe.declarations {
                        for (declaration, importance) in
                            declaration.block.declaration_importance_iter()
                        {
                            declarations.push(declaration.clone(), importance);
                        }
                    }
                    ScriptKeyframe {
                        offset: keyframe.computed_offset,
                        timing_function: keyframe.timing_function.clone(),
                        composite: match keyframe.composite {
                            CompositeOperationOrAuto::Replace => {
                                Some(AnimationComposition::Replace)
                            },
                            CompositeOperationOrAuto::Add => Some(AnimationComposition::Add),
                            CompositeOperationOrAuto::Accumulate => {
                                Some(AnimationComposition::Accumulate)
                            },
                            CompositeOperationOrAuto::Auto => None,
                        },
                        declarations: Arc::new(declarations),
                    }
                })
                .collect(),
        );
        *cached = Some(keyframes.clone());
        keyframes
    }

    /// The [target properties](https://drafts.csswg.org/web-animations-1/#target-property)
    /// of this effect, as physical longhands and custom properties, with logical properties
    /// mapped to physical ones by the writing mode of the target. Returns `None` when the
    /// keyframes come from style.
    pub(crate) fn target_properties(
        &self,
        writing_mode: WritingMode,
    ) -> Option<PropertyDeclarationIdSet> {
        if self.css_keyframes.get() {
            return None;
        }
        let mut properties = PropertyDeclarationIdSet::default();
        for keyframe in self.keyframes.borrow().iter() {
            for declaration in &keyframe.declarations {
                for (declaration, _) in declaration.block.declaration_importance_iter() {
                    match declaration.id() {
                        PropertyDeclarationId::Longhand(longhand) => properties.insert(
                            PropertyDeclarationId::Longhand(longhand.to_physical(writing_mode)),
                        ),
                        id => properties.insert(id),
                    };
                }
            }
        }
        Some(properties)
    }

    /// Note that the keyframes, target or composite operation of this effect changed, so
    /// that its animation updates the style of its target.
    fn effect_changed(&self, cx: &mut JSContext) {
        *self.keyframes_for_style.borrow_mut() = None;
        if let Some(animation) = self.upcast::<AnimationEffect>().associated_animation() {
            animation.effect_timing_changed(cx);
        }
    }
}

impl KeyframeEffectMethods<crate::DomTypeHolder> for KeyframeEffect {
    /// <https://drafts.csswg.org/web-animations-1/#dom-keyframeeffect-keyframeeffect>
    fn Constructor(
        cx: &mut JSContext,
        window: &Window,
        _: Option<HandleObject>,
        target: Option<&Element>,
        keyframes: *mut JSObject,
        options: UnrestrictedDoubleOrKeyframeEffectOptions,
    ) -> Fallible<DomRoot<KeyframeEffect>> {
        // Step 1. Create a new KeyframeEffect object, effect.
        let effect = KeyframeEffect::new(cx, window);

        // Step 2. Set the target element of effect to target.
        effect.target_element.set(target);

        // TODO: Step 3. Set the target pseudo-selector to the result corresponding to
        // the first matching condition below:

        // Step 4. Let timing input be the result corresponding to the first matching
        // condition below:
        let timing_input = match &options {
            // > If options is a KeyframeEffectOptions object,
            // > Let timing input be options.
            UnrestrictedDoubleOrKeyframeEffectOptions::KeyframeEffectOptions(options) => {
                let timing = &options.parent;
                OptionalEffectTiming {
                    delay: Some(timing.delay),
                    direction: Some(timing.direction),
                    duration: Some(match &timing.duration {
                        UnrestrictedDoubleOrString::UnrestrictedDouble(duration) => {
                            UnrestrictedDoubleOrString::UnrestrictedDouble(*duration)
                        },
                        UnrestrictedDoubleOrString::String(duration) => {
                            UnrestrictedDoubleOrString::String(duration.clone())
                        },
                    }),
                    easing: Some(timing.easing.clone()),
                    endDelay: Some(timing.endDelay),
                    fill: Some(timing.fill),
                    iterationStart: Some(timing.iterationStart),
                    iterations: Some(timing.iterations),
                }
            },
            // > Otherwise, if options is a double,
            // > Let timing input be a new EffectTiming object with all members set to their
            // > default values and duration set to options.
            UnrestrictedDoubleOrKeyframeEffectOptions::UnrestrictedDouble(duration) => {
                OptionalEffectTiming {
                    delay: Some(Finite::wrap(0.0)),
                    direction: Some(PlaybackDirection::Normal),
                    duration: Some(UnrestrictedDoubleOrString::UnrestrictedDouble(*duration)),
                    easing: Some(DOMString::from_static("linear")),
                    endDelay: Some(Finite::wrap(0.0)),
                    fill: Some(FillMode::Auto),
                    iterationStart: Some(Finite::wrap(0.0)),
                    iterations: Some(1.0),
                }
            },
        };

        // Step 5. Call the procedure to update the timing properties of an animation effect of
        // effect from timing input.
        // If that procedure causes an exception to be thrown, propagate the exception and abort this procedure.
        effect
            .upcast::<AnimationEffect>()
            .update_the_timing_properties(&timing_input)?;

        // Step 6. If options is a KeyframeEffectOptions object, assign the composite property of effect
        // to the corresponding value from options.
        if let UnrestrictedDoubleOrKeyframeEffectOptions::KeyframeEffectOptions(options) = &options
        {
            effect.composite.set(options.composite);
            // Step 3. Set the target pseudo-selector from the pseudoElement member of options.
            *effect.target_pseudo_selector.borrow_mut() =
                normalize_pseudo_selector(options.pseudoElement.clone())?;
        }

        // Step 7. Initialize the set of keyframes by performing the procedure defined for setKeyframes()
        // passing keyframes as the input.
        effect.SetKeyframes(cx, keyframes)?;

        Ok(effect)
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-keyframeeffect-target>
    fn GetTarget(&self) -> Option<DomRoot<Element>> {
        self.target_element.get()
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-keyframeeffect-target>
    fn SetTarget(&self, cx: &mut JSContext, target: Option<&Element>) {
        self.target_element.set(target);
        self.effect_changed(cx);
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-keyframeeffect-pseudoelement>
    fn GetPseudoElement(&self) -> Option<DOMString> {
        self.target_pseudo_selector.borrow().clone()
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-keyframeeffect-pseudoelement>
    fn SetPseudoElement(
        &self,
        cx: &mut JSContext,
        pseudo_element: Option<DOMString>,
    ) -> Fallible<()> {
        *self.target_pseudo_selector.borrow_mut() = normalize_pseudo_selector(pseudo_element)?;
        self.effect_changed(cx);
        Ok(())
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-keyframeeffect-composite>
    fn Composite(&self) -> CompositeOperation {
        self.composite.get()
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-keyframeeffect-composite>
    fn SetComposite(&self, cx: &mut JSContext, composite: CompositeOperation) {
        self.composite.set(composite);
        self.effect_changed(cx);
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-keyframeeffect-getkeyframes>
    fn GetKeyframes(
        &self,
        cx: &mut JSContext,
        result: &mut RootedVec<'_, Box<Heap<*mut JSObject>>>,
    ) -> Fallible<()> {
        let window = self.upcast::<AnimationEffect>().window();

        // Step 1. Let result be an empty sequence of objects.
        debug_assert!(result.is_empty());

        // > If this keyframe effect is associated with a CSSAnimation, and its keyframes have
        // > not been replaced by a successful call to setKeyframes(), the computed keyframes
        // > for this keyframe effect.
        if self.css_keyframes.get() {
            let keyframes = self
                .upcast::<AnimationEffect>()
                .associated_animation()
                .and_then(|animation| animation.css_computed_keyframes(cx))
                .unwrap_or_default();
            for keyframe in keyframes {
                let base_keyframe = BaseComputedKeyframe {
                    composite: keyframe.composite,
                    offset: Some(Finite::wrap(keyframe.offset)),
                    computedOffset: Some(Finite::wrap(keyframe.offset)),
                    easing: keyframe.easing,
                };
                rooted!(&in(cx) let mut output_keyframe = JS_NewObject(cx, None));
                base_keyframe.to_jsobject(cx, output_keyframe.handle_mut());
                for (property_name, value_string) in keyframe.values {
                    rooted!(&in(cx) let mut value = UndefinedValue());
                    value_string.to_jsval(cx, value.handle_mut());
                    let Ok(property_name) = CString::new(property_name) else {
                        continue;
                    };
                    if !JS_DefineProperty(
                        cx,
                        output_keyframe.handle(),
                        &property_name,
                        value.handle(),
                        PropertyFlags::ENUMERABLE |
                            PropertyFlags::WRITABLE |
                            PropertyFlags::CONFIGURABLE,
                    ) {
                        return Err(Error::Operation(Some(
                            "Could not define a property of a keyframe".to_owned(),
                        )));
                    }
                }
                result.push(Heap::boxed(output_keyframe.get()))
            }
            return Ok(());
        }

        // Step 2. Let keyframes be one of the following:
        // If this keyframe effect is associated with a CSSAnimation, and its keyframes have not been replaced
        // by a successful call to setKeyframes(),
        // the computed keyframes for this keyframe effect.
        // Otherwise,
        // the result of applying the procedure compute missing keyframe offsets to the keyframes for this keyframe effect.
        // TODO: We don't compute missing keyframe offsets yet. But that will likely happen in stylo, not here.
        let keyframes = self.keyframes.borrow();

        // Step 3. For each keyframe in keyframes perform the following steps:
        for keyframe in keyframes.iter() {
            // Step 3.1 Initialize a dictionary object, output keyframe, using the following definition:
            // TODO Step 3.2 Set the offset, computedOffset, easing, and composite members of output keyframe
            // to the respective keyframe offset, computed keyframe offset, keyframe-specific easing function,
            // and keyframe-specific composite operation values of keyframe.
            let base_keyframe = BaseComputedKeyframe {
                composite: keyframe.composite,
                offset: keyframe.offset,
                // FIXME: We don't post-process the offset of keyframes to find suitable offset values for null
                // keyframe offsets yet, so we just use the offset as-is.
                computedOffset: Some(Finite::wrap(keyframe.computed_offset)),
                easing: keyframe.easing_function.clone(),
            };
            rooted!(&in(cx) let mut output_keyframe = JS_NewObject(cx, None));
            base_keyframe.to_jsobject(cx, output_keyframe.handle_mut());

            // Step 3.3 For each animation property-value pair declaration in keyframe, perform the following steps:
            for property_value_pair in &keyframe.declarations {
                debug_assert!(property_value_pair.property_id.is_animatable());

                // Step 3.3.1 Let property name be the result of applying the animation property name to IDL attribute
                // name algorithm to the property name of declaration.
                let mut property_name = String::new();
                let mut writer = CssWriter::new(&mut property_name);
                if property_value_pair.property_id.to_css(&mut writer).is_err() {
                    continue;
                }
                let property_name = animation_property_name_to_idl_attribute_name(&property_name);

                // Step 3.3.2 Let IDL value be the result of serializing the property value of declaration
                // by passing declaration to the algorithm to serialize a CSS value [CSSOM].
                let mut value_string = String::new();
                let is_single_value_value_to_css = {
                    let mut layout = window.layout_mut();
                    property_value_pair.block.single_value_to_css(
                        &property_value_pair.property_id,
                        &mut value_string,
                        None,
                        layout.stylist_mut(),
                    )
                };
                if is_single_value_value_to_css.is_err() {
                    continue;
                }

                // Step 3.3.3 Let value be the result of converting IDL value to an ECMAScript String value.
                rooted!(&in(cx) let mut value = UndefinedValue());
                value_string.to_jsval(cx, value.handle_mut());

                // Step 3.3.4 Call the [[DefineOwnProperty]] internal method on output keyframe with property
                // name property name, Property Descriptor { [[Writable]]: true, [[Enumerable]]: true, [[Configurable]]:
                // true, [[Value]]: value } and Boolean flag false.
                let Ok(property_name) = CString::new(property_name) else {
                    continue;
                };

                let success = JS_DefineProperty(
                    cx,
                    output_keyframe.handle(),
                    &property_name,
                    value.handle(),
                    PropertyFlags::ENUMERABLE |
                        PropertyFlags::WRITABLE |
                        PropertyFlags::CONFIGURABLE,
                );
                if !success {
                    if cfg!(debug_assertions) {
                        unreachable!("Setting a property on output_keyframe should never fail");
                    }
                    return Err(Error::Operation(None));
                }
            }

            // Step 3.4 Append output keyframe to result.
            result.push(Heap::boxed(output_keyframe.get()))
        }

        // Step 4. Return result.
        Ok(())
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-keyframeeffect-setkeyframes>
    fn SetKeyframes(&self, cx: &mut JSContext, keyframes: *mut JSObject) -> Fallible<()> {
        // > This effect’s set of keyframes is replaced with the result of performing the procedure to
        // > process a keyframes argument. If that procedure throws an exception, this effect’s
        // > keyframes are not modified.
        let document = self.upcast::<AnimationEffect>().window().Document();
        let keyframes = process_a_keyframes_argument(cx, &document, keyframes)?;
        *self.keyframes.safe_borrow_mut(cx.no_gc()) = keyframes;
        // > After a successful call to setKeyframes() on the effect of a CSSAnimation,
        // > changes to @keyframes rules are no longer reflected in that animation.
        self.css_keyframes.set(false);
        self.effect_changed(cx);
        Ok(())
    }
}

/// <https://drafts.csswg.org/web-animations-1/#process-a-keyframes-argument>
fn process_a_keyframes_argument(
    cx: &mut JSContext,
    document: &Document,
    keyframes: *mut JSObject,
) -> Fallible<Vec<Keyframe>> {
    // Step 1. If object is null, return an empty sequence of keyframes.
    if keyframes.is_null() {
        return Ok(Vec::new());
    }

    // Step 2. Let processed keyframes be an empty sequence of keyframes.

    // Step 3. Let method be the result of GetMethod(object, @@iterator).
    // Step 4. Check the completion record of method.
    // Step 5. Perform the steps corresponding to the first matching condition below:
    rooted!(&in(cx) let iterable = ObjectValue(keyframes));
    let mut keyframes = Vec::new();
    let result = for_of(cx, iterable.handle(), |cx, iterator_element| {
        // Step 5.3.4 Let nextItem be IteratorValue(next).
        // Step 5.3.5 Check the completion record of nextItem.
        // Note: This happens inside the "for_of" call.

        // Step 5.3.6 If Type(nextItem) is not Undefined, Null or Object, then throw a TypeError
        // and abort these steps.
        if !iterator_element.is_null_or_undefined() && !iterator_element.is_object() {
            return Err(ForOfIterationFailure::Other(Error::Type(
                c"Keyframe must be an object, null or undefined".to_owned(),
            )));
        }

        // Step 5.3.7 Append to processed keyframes the result of running the procedure to process a
        // keyframe-like object passing nextItem as the keyframe input with the allow lists flag set to false.
        keyframes.push(process_a_keyframe_like_object(
            cx,
            document,
            iterator_element,
        )?);

        Ok(ControlFlow::Continue(()))
    });
    match result {
        Ok(()) => {
            check_keyframe_offsets(&keyframes)?;
            compute_missing_keyframe_offsets(&mut keyframes);
            Ok(keyframes)
        },
        Err(ForOfIterationFailure::ValueIsNotIterable) => {
            let mut keyframes =
                process_a_property_indexed_keyframe(cx, document, iterable.handle())?;
            check_keyframe_offsets(&keyframes)?;
            compute_missing_keyframe_offsets(&mut keyframes);
            Ok(keyframes)
        },
        Err(ForOfIterationFailure::JSFailed) => Err(Error::JSFailed),
        Err(ForOfIterationFailure::Other(error)) => Err(error),
    }
}

/// Steps 6 and 7 of <https://drafts.csswg.org/web-animations-1/#process-a-keyframes-argument>.
fn check_keyframe_offsets(keyframes: &[Keyframe]) -> Fallible<()> {
    // Step 6. If processed keyframes is not loosely sorted by offset, throw a TypeError
    // and abort these steps.
    // Step 7. If there exist any keyframe in processed keyframes whose keyframe offset is
    // non-null and less than zero or greater than one, throw a TypeError and abort these steps.
    let mut previous_offset = f64::NEG_INFINITY;
    for keyframe in keyframes.iter() {
        let Some(offset) = keyframe.offset else {
            continue;
        };
        let offset = *offset;
        if !(0.0..=1.0).contains(&offset) {
            return Err(Error::Type(
                c"Keyframe offsets must be between 0 and 1".to_owned(),
            ));
        }
        if offset < previous_offset {
            return Err(Error::Type(
                c"Keyframe offsets must be loosely sorted".to_owned(),
            ));
        }
        previous_offset = offset;
    }
    Ok(())
}

/// The "Otherwise" branch of step 5 of
/// <https://drafts.csswg.org/web-animations-1/#process-a-keyframes-argument>, for a
/// property-indexed keyframe object.
fn process_a_property_indexed_keyframe(
    cx: &mut JSContext,
    document: &Document,
    object: HandleValue,
) -> Fallible<Vec<Keyframe>> {
    // Step 5.1. Let property-indexed keyframe be the result of running the procedure to process
    // a keyframe-like object passing object as the keyframe input and with the allow lists flag
    // set to true.
    let Ok(base) = BasePropertyIndexedKeyframe::from_jsval(cx, object, ()) else {
        return Err(Error::JSFailed);
    };
    let base = match base {
        ConversionResult::Success(base) => base,
        ConversionResult::Failure(error_message) => {
            return Err(Error::Type(error_message.into_owned()));
        },
    };
    rooted!(&in(cx) let object = object.to_object());
    let values = get_property_values(cx, object.handle(), true)?;

    // Step 5.2. For each member, m, in property-indexed keyframe, perform the following steps:
    let mut keyframes: Vec<Keyframe> = Vec::new();
    let linear = parse_easing(document, "linear").expect("linear should be a valid easing");
    for (property_id, property_values) in values {
        // Step 5.2.1. Let property name be the key for m.
        // Step 5.2.2. If property name is "composite", or "easing", or "offset", skip the
        // remaining steps in this loop and continue from the next member in property-indexed
        // keyframe after m.
        // Step 5.2.3. Let property values be the value for m.
        // Step 5.2.4. Let property keyframes be an empty sequence of keyframes.
        let count = property_values.len();
        // Step 5.2.5. For each value, v, in property values perform the following steps:
        for (index, value) in property_values.into_iter().enumerate() {
            // Step 5.2.5.1. Let k be a new keyframe with a null keyframe offset.
            // Step 5.2.5.2. Add the property-value pair, property name → v, to k.
            // Step 5.2.5.3. Append k to property keyframes.
            // Step 5.2.6. Apply the procedure to compute missing keyframe offsets to property
            // keyframes.
            let computed_offset = if count == 1 {
                1.0
            } else {
                index as f64 / (count - 1) as f64
            };
            // Step 5.2.7. Add keyframes in property keyframes to processed keyframes.
            keyframes.push(Keyframe {
                offset: None,
                computed_offset,
                easing_function: DOMString::from_static("linear"),
                timing_function: linear.clone(),
                composite: CompositeOperationOrAuto::Auto,
                declarations: parse_property_values(document, vec![(property_id.clone(), value)]),
            });
        }
    }

    // Step 5.3. Sort processed keyframes by the computed keyframe offset of each keyframe in
    // increasing order.
    keyframes.sort_by(|a, b| a.computed_offset.total_cmp(&b.computed_offset));

    // Step 5.4. Merge adjacent keyframes in processed keyframes when they have equal computed
    // keyframe offsets.
    let mut merged: Vec<Keyframe> = Vec::with_capacity(keyframes.len());
    for keyframe in keyframes {
        match merged.last_mut() {
            Some(last) if last.computed_offset == keyframe.computed_offset => {
                last.declarations.extend(keyframe.declarations);
            },
            _ => merged.push(keyframe),
        }
    }
    let mut keyframes = merged;

    // Step 5.5. Let offsets be a sequence of nullable double values assigned based on the type
    // of the "offset" member of the property-indexed keyframe as follows:
    let offsets: Vec<Option<Finite<f64>>> = match base.offset {
        None => vec![],
        Some(None) => vec![None],
        Some(Some(DoubleOrDoubleOrNullSequence::Double(offset))) => vec![Some(offset)],
        Some(Some(DoubleOrDoubleOrNullSequence::DoubleOrNullSequence(offsets))) => offsets,
    };

    // Step 5.6. Assign each value in offsets to the keyframe offset of the keyframe with the
    // corresponding position in processed keyframes until the end of either sequence is reached.
    for (keyframe, offset) in keyframes.iter_mut().zip(offsets) {
        keyframe.offset = offset;
    }

    // Step 5.7. Let easings be a sequence of DOMString values assigned based on the type of the
    // "easing" member of the property-indexed keyframe as follows:
    let mut easings = match base.easing {
        None => vec![],
        Some(StringOrStringSequence::String(easing)) => vec![easing],
        Some(StringOrStringSequence::StringSequence(easings)) => easings,
    };
    // Step 5.8. If easings is an empty sequence, let it be a sequence consisting of a single
    // value, "linear".
    if easings.is_empty() {
        easings.push(DOMString::from_static("linear"));
    }
    // Step 5.9. If easings has fewer items than processed keyframes, repeat the elements in
    // easings successively starting from the beginning of the list until easings has as many
    // items as processed keyframes.
    // Step 5.10. If easings has more items than processed keyframes, store the excess items as
    // unused easings.
    // Step 5.11. Assign each value in easings to a property named "easing" on the keyframe with
    // the corresponding position in processed keyframes until the end of processed keyframes is
    // reached.
    // Step 8 (parsing the easing functions, also of unused easings).
    let timing_functions = easings
        .iter()
        .map(|easing| {
            parse_easing(document, &easing.str())
                .ok_or_else(|| Error::Type(c"\"easing\" is not a valid timing function".to_owned()))
        })
        .collect::<Fallible<Vec<_>>>()?;
    for (index, keyframe) in keyframes.iter_mut().enumerate() {
        keyframe.easing_function = easings[index % easings.len()].clone();
        keyframe.timing_function = timing_functions[index % easings.len()].clone();
    }

    // Step 5.12. If the "composite" member of the property-indexed keyframe is not an empty
    // sequence:
    let composites = match base.composite {
        None => vec![],
        Some(CompositeOperationOrAutoOrCompositeOperationOrAutoSequence::CompositeOperationOrAuto(
            composite,
        )) => vec![composite],
        Some(
            CompositeOperationOrAutoOrCompositeOperationOrAutoSequence::CompositeOperationOrAutoSequence(
                composites,
            ),
        ) => composites,
    };
    if !composites.is_empty() {
        // Step 5.12.1-3. Repeat the composite modes as for the easings and assign them.
        for (index, keyframe) in keyframes.iter_mut().enumerate() {
            keyframe.composite = composites[index % composites.len()];
        }
    }

    Ok(keyframes)
}

/// <https://drafts.csswg.org/web-animations-1/#keyframe>
#[derive(JSTraceable, MallocSizeOf)]
struct Keyframe {
    offset: Option<Finite<f64>>,
    /// <https://drafts.csswg.org/web-animations-1/#computed-keyframe-offset>
    computed_offset: f64,
    easing_function: DOMString,
    /// The parsed `easing_function`.
    #[no_trace]
    timing_function: ComputedTimingFunction,
    composite: CompositeOperationOrAuto,
    declarations: Vec<KeyframePropertyDeclaration>,
}

#[derive(JSTraceable, MallocSizeOf)]
struct KeyframePropertyDeclaration {
    #[no_trace]
    property_id: PropertyId,
    /// The block is known to only contain declarations for `property_id`. There might
    /// be more than one value if `property_id` is a shorthand.
    #[no_trace]
    block: PropertyDeclarationBlock,
}

/// <https://drafts.csswg.org/web-animations-1/#process-a-keyframe-like-object>
fn process_a_keyframe_like_object(
    cx: &mut JSContext,
    document: &Document,
    keyframe_input: HandleValue,
) -> Fallible<Keyframe> {
    // Step 1. Run the procedure to convert an ECMAScript value to a dictionary type [WEBIDL] with keyframe input
    // as the ECMAScript value, and the dictionary type depending on the value of the allow lists flag as follows:
    // If allow lists is true,
    // Use the following dictionary type:
    //
    // dictionary BasePropertyIndexedKeyframe {
    //   (double? or sequence<double?>)                         offset = [];
    //   (DOMString or sequence<DOMString>)                     easing = [];
    //   (CompositeOperationOrAuto or sequence<CompositeOperationOrAuto>) composite = [];
    // };
    //
    // Otherwise,
    //     Use the following dictionary type:
    //
    //     dictionary BaseKeyframe {
    //       double?                  offset = null;
    //       DOMString                easing = "linear";
    //       CompositeOperationOrAuto composite = "auto";
    //     };
    //
    // Store the result of this procedure as keyframe output.
    //
    // Note: The property-indexed case, where allow lists is true, is in
    // `process_a_property_indexed_keyframe`.
    let Ok(keyframe_output) = BaseKeyframe::from_jsval(cx, keyframe_input, ()) else {
        return Err(Error::JSFailed);
    };
    let ConversionResult::Success(keyframe_output) = keyframe_output else {
        return Err(Error::Operation(None));
    };
    // Step 8 of <https://drafts.csswg.org/web-animations-1/#process-a-keyframes-argument>:
    // > For each frame in processed keyframes, perform the following steps: [..] Let the timing
    // > function of frame be the result of parsing the "easing" property on frame using the
    // > CSS syntax defined for the easing member of the EffectTiming dictionary.
    // > If parsing the "easing" property fails, throw a TypeError and abort this procedure.
    let timing_function = parse_easing(document, &keyframe_output.easing.str())
        .ok_or_else(|| Error::Type(c"\"easing\" is not a valid timing function".to_owned()))?;
    let mut keyframe_output = Keyframe {
        offset: keyframe_output.offset,
        computed_offset: 0.0,
        easing_function: keyframe_output.easing,
        timing_function,
        composite: keyframe_output.composite,
        declarations: Vec::new(),
    };

    // Step 2. If keyframe input is null or undefined, return keyframe output.
    if keyframe_input.is_null_or_undefined() {
        return Ok(keyframe_output);
    }

    // From Step 3 onwards our implementation diverges from the specification. The spec
    // wants us to build a list of animatable CSS properties and a list of properties on
    // the object, then compute the union between the two.
    //
    // Instead, we iterate over all properties on the object and then check if they correspond
    // to an animatable property.
    rooted!(&in(cx) let object = keyframe_input.to_object());

    // Steps 2 - 6 are in get_property_values
    let values = get_property_values(cx, object.handle(), false)?
        .into_iter()
        .filter_map(|(property_id, mut values)| Some((property_id, values.pop()?)))
        .collect();
    keyframe_output.declarations = parse_property_values(document, values);

    // Step 7. Return keyframe output.
    Ok(keyframe_output)
}

/// Implements Step 2-6 of  <https://drafts.csswg.org/web-animations-1/#process-a-keyframe-like-object>.
#[expect(unsafe_code)]
fn get_property_values(
    cx: &mut JSContext,
    object: HandleObject,
    allow_lists: bool,
) -> Fallible<Vec<(PropertyId, Vec<DOMString>)>> {
    // The spec tells us to iterate over all animatable properties and see if they're defined
    // on the object. Instead we can iterate over the own properties of the object and see
    // if they're animated properties, that's easier.
    let mut ids = IdVector::new(cx);
    if !GetPropertyKeys(cx, object, JSITER_OWNONLY, ids.handle_mut()) {
        return Ok(Vec::new());
    }

    let mut values = Vec::with_capacity(ids.len());
    for id in ids.iter() {
        rooted!(&in(cx) let id = *id);

        // See if the id for the current property on the object represents a animatable CSS property.
        if !id.is_string() {
            continue;
        }
        rooted!(&in(cx) let mut key_value = UndefinedValue());
        if !JS_IdToValue(cx, id.handle().get(), key_value.handle_mut()) {
            continue;
        }
        rooted!(&in(cx) let js_string = key_value.to_string());
        let Some(js_string) = NonNull::new(js_string.get()) else {
            continue;
        };
        let property_name = unsafe { jsstr_to_string(cx, js_string) };

        let property_id = if property_name.starts_with("--") {
            PropertyId::parse_enabled_for_all_content(&property_name)
                .ok()
                .filter(|id| matches!(id, PropertyId::Custom(_)))
        } else {
            lookup_css_property_by_idl_attribute_name(&property_name).map(PropertyId::NonCustom)
        };
        let Some(property_id) = property_id else {
            continue;
        };
        debug_assert!(property_id.is_animatable());

        // Step 6.1 Let raw value be the result of calling the [[Get]] internal method on keyframe input,
        // with property name as the property key and keyframe input as the receiver.
        // Step 6.2 Check the completion record of raw value.
        rooted!(&in(cx) let mut property_value = UndefinedValue());
        if !unsafe {
            JS_GetPropertyById(
                cx.raw_cx(),
                object.into_handle(),
                id.handle().into_handle(),
                property_value.handle_mut().into_handle_mut(),
            )
        } {
            continue;
        }

        // Step 6.3 Convert raw value to a DOMString or to a sequence of DOMStrings property values as follows:
        // If allow lists is true,
        // Let property values be the result of converting raw value to IDL type (DOMString or
        // sequence<DOMString>) using the procedures defined for converting an ECMAScript value to an
        // IDL value [WEBIDL].
        // If property values is a single DOMString, replace property values with a sequence of
        // DOMStrings with the original value of property values as the only element.
        if allow_lists {
            let property_values =
                match StringOrStringSequence::from_jsval(cx, property_value.handle(), ()) {
                    Ok(ConversionResult::Success(StringOrStringSequence::String(value))) => {
                        vec![value]
                    },
                    Ok(ConversionResult::Success(StringOrStringSequence::StringSequence(
                        values,
                    ))) => values,
                    Ok(ConversionResult::Failure(error_message)) => {
                        return Err(Error::Type(error_message.into_owned()));
                    },
                    Err(_) => return Err(Error::JSFailed),
                };
            values.push((property_id, property_values));
            continue;
        }

        // Otherwise,
        // Let property values be the result of converting raw value to a DOMString using the procedure
        // for converting an ECMAScript value to a DOMString [WEBIDL].
        let property_value = match DOMString::from_jsval(
            cx,
            property_value.handle(),
            StringificationBehavior::Default,
        ) {
            Ok(ConversionResult::Success(property_value)) => property_value,
            Ok(ConversionResult::Failure(error_message)) => {
                return Err(Error::Operation(
                    error_message
                        .to_str()
                        .ok()
                        .map(|message| message.to_owned()),
                ));
            },
            Err(_) => return Err(Error::JSFailed),
        };

        // Step 6.4 Calculate the normalized property name as the result of applying the IDL attribute name
        // to animation property name algorithm to property name.
        // Note: Due to the way our implementation differs from the spec (refer to the comment at
        // the top of this function), we already have the normalized property name.

        // Step 6.5 Add a property to keyframe output with normalized property name as the property name,
        // and property values as the property value.
        values.push((property_id, vec![property_value]));
    }

    Ok(values)
}

/// Parse the values of a keyframe-like object, dropping the ones that are not valid.
/// The specification keeps them as strings, but invalid values have no effect.
fn parse_property_values(
    document: &Document,
    values: Vec<(PropertyId, DOMString)>,
) -> Vec<KeyframePropertyDeclaration> {
    let urlextradata = document.url().into_url().into();
    let mut parser_context = parser_context_for_document(
        document,
        CssRuleType::Style,
        ParsingMode::DEFAULT,
        &urlextradata,
    );
    values
        .into_iter()
        .filter_map(|(property_id, value)| {
            parse_single_property_declaration(property_id, &value.str(), &mut parser_context)
        })
        .collect()
}

/// Check a target pseudo-selector: null, or a pseudo-element selector. The legacy
/// single-colon forms of `:before`, `:after`, `:first-letter` and `:first-line` are
/// turned into their double-colon forms.
///
/// <https://drafts.csswg.org/web-animations-1/#dom-keyframeeffect-pseudoelement>
pub(crate) fn normalize_pseudo_selector(
    selector: Option<DOMString>,
) -> Fallible<Option<DOMString>> {
    let Some(selector) = selector else {
        return Ok(None);
    };
    let string = selector.str().to_string();
    let lower = string.to_ascii_lowercase();
    let canonical = match lower.as_str() {
        ":before" | ":after" | ":first-letter" | ":first-line" => format!(":{lower}"),
        _ => lower,
    };
    if pseudo_element_from_selector(&canonical).is_some() {
        return Ok(Some(DOMString::from(canonical)));
    }
    if let Some((name, argument)) = string.split_once('(') {
        // Functional pseudo-element arguments can be case-sensitive custom identifiers.
        let canonical = format!("{}({argument}", name.to_ascii_lowercase());
        if valid_functional_pseudo_selector(&canonical) {
            return Ok(Some(DOMString::from(canonical)));
        }
    }
    Err(Error::Syntax(None))
}

fn valid_functional_pseudo_selector(selector: &str) -> bool {
    let Some((name, argument)) = selector.split_once('(') else {
        return false;
    };
    if !matches!(name, "::part" | "::highlight") {
        return false;
    }
    let Some(argument) = argument.strip_suffix(')') else {
        return false;
    };
    let mut parser = Parser::new(argument);
    if parser.expect_ident().is_err() {
        return false;
    }
    if name == "::part" {
        while !parser.is_exhausted() {
            if parser.expect_ident().is_err() {
                return false;
            }
        }
    }
    parser.is_exhausted()
}

fn pseudo_element_from_selector(selector: &str) -> Option<PseudoElement> {
    Some(match selector {
        "::before" => PseudoElement::Before,
        "::after" => PseudoElement::After,
        "::selection" => PseudoElement::Selection,
        "::first-letter" => PseudoElement::FirstLetter,
        "::first-line" => PseudoElement::FirstLine,
        "::backdrop" => PseudoElement::Backdrop,
        "::details-content" => PseudoElement::DetailsContent,
        "::marker" => PseudoElement::Marker,
        "::color-swatch" => PseudoElement::ColorSwatch,
        "::file-selector-button" => PseudoElement::FileSelectorButton,
        "::placeholder" => PseudoElement::Placeholder,
        "::slider-fill" => PseudoElement::SliderFill,
        "::slider-thumb" => PseudoElement::SliderThumb,
        "::slider-track" => PseudoElement::SliderTrack,
        "::-moz-progress-bar" => PseudoElement::MozProgressBar,
        _ => return None,
    })
}

/// Parse an easing function, as for the `easing` member of `EffectTiming`.
fn parse_easing(document: &Document, input: &str) -> Option<ComputedTimingFunction> {
    let urlextradata = document.url().into_url().into();
    let parser_context = parser_context_for_document(
        document,
        CssRuleType::Style,
        ParsingMode::DEFAULT,
        &urlextradata,
    );
    let mut parser = Parser::new(input);
    parser
        .parse_entirely(|parser| SpecifiedTimingFunction::parse(&parser_context, parser))
        .ok()
        .map(|timing_function| timing_function.to_computed_value_without_context())
}

/// <https://drafts.csswg.org/web-animations-1/#compute-missing-keyframe-offsets>
fn compute_missing_keyframe_offsets(keyframes: &mut [Keyframe]) {
    // Step 1. For each keyframe, in keyframes, let the computed keyframe offset of the
    // keyframe be equal to its keyframe offset value.
    let mut computed_offsets: Vec<Option<f64>> = keyframes
        .iter()
        .map(|keyframe| keyframe.offset.map(|offset| *offset))
        .collect();

    // Step 2. If keyframes contains more than one keyframe and the computed keyframe offset
    // of the first keyframe in keyframes is null, set the computed keyframe offset of the
    // first keyframe to 0.
    if computed_offsets.len() > 1 && computed_offsets[0].is_none() {
        computed_offsets[0] = Some(0.0);
    }

    // Step 3. If the computed keyframe offset of the last keyframe in keyframes is null, set
    // its computed keyframe offset to 1.
    if let Some(last) = computed_offsets.last_mut() {
        if last.is_none() {
            *last = Some(1.0);
        }
    }

    // Step 4. For each pair of keyframes A and B where A appears before B in keyframes, and
    // A and B have a computed keyframe offset that is not null, and all keyframes between A
    // and B have a null computed keyframe offset, calculate the computed keyframe offset of
    // each keyframe between A and B as follows: offsetk = offsetA + (offsetB − offsetA) × k / n
    let mut previous: Option<usize> = None;
    for index in 0..computed_offsets.len() {
        let Some(offset_b) = computed_offsets[index] else {
            continue;
        };
        if let Some(a) = previous {
            let offset_a = computed_offsets[a].unwrap_or_default();
            let n = (index - a) as f64;
            for k in 1..(index - a) {
                computed_offsets[a + k] = Some(offset_a + (offset_b - offset_a) * k as f64 / n);
            }
        }
        previous = Some(index);
    }

    for (keyframe, offset) in keyframes.iter_mut().zip(computed_offsets) {
        keyframe.computed_offset = offset.unwrap_or_default();
    }
}

/// Parses `input` as a value for `property`, returning `None` on failure.
fn parse_single_property_declaration(
    property: PropertyId,
    input: &str,
    parser_context: &mut ParserContext<'_>,
) -> Option<KeyframePropertyDeclaration> {
    let mut declaration = SourcePropertyDeclaration::default();
    let mut parser = Parser::new(input);

    // TODO: Consider reporting parse errors somewhere useful, like the devtools console.
    parser
        .parse_entirely(|parser| {
            PropertyDeclaration::parse_into(
                &mut declaration,
                property.clone(),
                parser_context,
                parser,
            )
        })
        .ok()?;

    let mut block = PropertyDeclarationBlock::new();
    block.extend(declaration.drain(), Importance::Normal);

    Some(KeyframePropertyDeclaration {
        property_id: property,
        block,
    })
}

/// This is the inverse of <https://drafts.csswg.org/web-animations-1/#animation-property-name-to-idl-attribute-name>.
fn lookup_css_property_by_idl_attribute_name(attribute_name: &str) -> Option<NonCustomPropertyId> {
    // Step 1, for custom properties, is handled by get_property_values before this lookup.

    // Steps 2 to 4 are in `animation_property_name_to_idl_attribute_name`, which maps
    // `float` to "cssFloat" and `offset` to "cssOffset", so "float" and "offset" (the
    // keyframe offset) never name a property here.
    static IDL_ATTRIBUTE_TO_ANIMATED_PROPERTY_LOOKUP_TABLE: LazyLock<
        FxHashMap<String, NonCustomPropertyId>,
    > = LazyLock::new(|| {
        log::debug!("Initializing map from IDL attribute names to CSS properties");

        NonCustomPropertyId::iter()
            .filter(|non_custom_property| non_custom_property.is_animatable())
            .filter(|non_custom_property| {
                non_custom_property
                    .to_property_id()
                    .enabled_for_all_content()
            })
            .map(|non_custom_property| {
                let idl_attribute_name =
                    animation_property_name_to_idl_attribute_name(non_custom_property.name());
                (idl_attribute_name, non_custom_property)
            })
            .collect()
    });
    IDL_ATTRIBUTE_TO_ANIMATED_PROPERTY_LOOKUP_TABLE
        .get(attribute_name)
        .copied()
}

/// <https://drafts.csswg.org/web-animations-1/#animation-property-name-to-idl-attribute-name>
///
/// # Panics
/// Panics when the property name is empty or consists only of `-` characters.
/// In general it is assumed that `property_name` is a CSS property.
pub(crate) fn animation_property_name_to_idl_attribute_name(property_name: &str) -> String {
    // Step 1. If property follows the <custom-property-name> production, return property.
    if property_name.starts_with("--") {
        return property_name.to_owned();
    }

    // Step 2. If property refers to the CSS float property, return the string "cssFloat".
    if property_name == "float" {
        return "cssFloat".to_owned();
    }

    // Step 3. If property refers to the CSS offset property, return the string "cssOffset".
    if property_name == "offset" {
        return "cssOffset".to_owned();
    }

    // Step 4. Otherwise, return the result of applying the CSS property to IDL attribute
    // algorithm [CSSOM] to property.
    let mut idl_attribute_name = String::with_capacity(property_name.len());

    let mut chunks = property_name.split('-');
    let Some(first_chunk) = chunks.next() else {
        unreachable!("CSS property name should not consist only of dashes");
    };
    idl_attribute_name.push_str(first_chunk);
    for chunk in chunks {
        let mut characters = chunk.chars();
        let Some(to_capitalize) = characters.next() else {
            continue;
        };
        idl_attribute_name.push(to_capitalize.to_ascii_uppercase());
        idl_attribute_name.push_str(characters.as_str());
    }

    idl_attribute_name
}
