//! Derive macros for [`bevy_action_map`](https://docs.rs/bevy_action_map).
//!
//! Use them like this:
//!
//! ```rust
//! use bevy_action_map::prelude::*;
//!
//! #[derive(bevy_action_map::InputAction)]
//! #[action(path = "gameplay.jump", output = bool, intent = Button)]
//! struct Jump;
//!
//! #[derive(bevy_action_map::InputContext)]
//! #[context(path = "gameplay.on_foot", tick = Fixed)]
//! struct OnFoot;
//! ```

#![forbid(unsafe_code)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Attribute, Data, DeriveInput, Error, Ident, LitInt, LitStr, Result, Type, parse_macro_input,
    spanned::Spanned,
};

#[proc_macro_derive(InputAction, attributes(action))]
/// Derives `bevy_action_map::action::InputAction` from `#[action(...)]` metadata.
pub fn derive_input_action(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_input_action(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(InputContext, attributes(context))]
/// Derives `bevy_action_map::action::InputContext` from `#[context(...)]` metadata.
pub fn derive_input_context(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_input_context(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

struct ActionArgs {
    output: Type,
    intent: Ident,
    path: LitStr,
}

struct ContextArgs {
    tick: Ident,
    priority: Option<LitInt>,
    path: LitStr,
}

fn expand_input_action(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    ensure_unit_struct(&input)?;
    let args = parse_action_args(&input.attrs, input.ident.span())?;
    let ident = input.ident;
    let path = args.path;

    let output = args.output;
    let intent = args.intent;

    Ok(quote! {
        impl ::bevy_action_map::action::InputAction for #ident {
            type Output = #output;
            const INTENT: ::bevy_action_map::action::Intent = ::bevy_action_map::action::Intent::#intent;
            const PATH: &'static str = #path;

            fn id() -> ::bevy_action_map::action::ActionId {
                // Rust has no generic statics, so the cache cannot live on the trait's default
                // method — but this impl is concrete, so it can hold one.
                static ID: ::bevy_action_map::action::ActionIdCache =
                    ::bevy_action_map::action::ActionIdCache::new();
                ID.get_or_intern(#path)
            }
        }
    })
}

fn expand_input_context(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    ensure_unit_struct(&input)?;
    let args = parse_context_args(&input.attrs, input.ident.span())?;
    let ident = input.ident;
    let path = args.path;

    let tick = args.tick;
    let priority = args
        .priority
        .unwrap_or_else(|| LitInt::new("0", ident.span()));

    Ok(quote! {
        impl ::bevy_action_map::action::InputContext for #ident {
            const TICK: ::bevy_action_map::action::TickDomain = ::bevy_action_map::action::TickDomain::#tick;
            const PRIORITY: i32 = #priority;
            const PATH: &'static str = #path;
        }
    })
}

fn ensure_unit_struct(input: &DeriveInput) -> Result<()> {
    if !input.generics.params.is_empty() {
        return Err(Error::new(
            input.generics.span(),
            "derive macros in bevy_action_map currently support only unit structs",
        ));
    }

    match &input.data {
        Data::Struct(data) if matches!(data.fields, syn::Fields::Unit) => Ok(()),
        Data::Struct(data) => Err(Error::new(
            data.fields.span(),
            "derive macros in bevy_action_map currently support only unit structs",
        )),
        _ => Err(Error::new(
            input.span(),
            "derive macros in bevy_action_map currently support only unit structs",
        )),
    }
}

fn parse_action_args(
    attributes: &[Attribute],
    missing_attr_span: proc_macro2::Span,
) -> Result<ActionArgs> {
    let mut output = None;
    let mut intent = None;
    let mut path = None;

    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("action"))
    {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("output") {
                let value: Type = meta.value()?.parse()?;
                output = Some(value);
                Ok(())
            } else if meta.path.is_ident("intent") {
                let value: Ident = meta.value()?.parse()?;
                intent = Some(value);
                Ok(())
            } else if meta.path.is_ident("path") {
                let value: LitStr = meta.value()?.parse()?;
                path = Some(value);
                Ok(())
            } else {
                Err(meta.error("unsupported #[action(...)] argument"))
            }
        })?;
    }

    let output =
        output.ok_or_else(|| Error::new(missing_attr_span, "missing #[action(output = ...)]"))?;
    let intent =
        intent.ok_or_else(|| Error::new(missing_attr_span, "missing #[action(intent = ...)]"))?;
    let path =
        path.ok_or_else(|| Error::new(missing_attr_span, "missing #[action(path = ...)]"))?;

    Ok(ActionArgs {
        output,
        intent,
        path,
    })
}

fn parse_context_args(
    attributes: &[Attribute],
    missing_attr_span: proc_macro2::Span,
) -> Result<ContextArgs> {
    let mut tick = None;
    let mut priority = None;
    let mut path = None;

    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("context"))
    {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("tick") {
                let value: Ident = meta.value()?.parse()?;
                tick = Some(value);
                Ok(())
            } else if meta.path.is_ident("priority") {
                let value: LitInt = meta.value()?.parse()?;
                priority = Some(value);
                Ok(())
            } else if meta.path.is_ident("path") {
                let value: LitStr = meta.value()?.parse()?;
                path = Some(value);
                Ok(())
            } else {
                Err(meta.error("unsupported #[context(...)] argument"))
            }
        })?;
    }

    let tick =
        tick.ok_or_else(|| Error::new(missing_attr_span, "missing #[context(tick = ...)]"))?;
    let path =
        path.ok_or_else(|| Error::new(missing_attr_span, "missing #[context(path = ...)]"))?;

    Ok(ContextArgs {
        tick,
        priority,
        path,
    })
}
