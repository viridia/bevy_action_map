//! Derive macros for [`bevy_action_map`](https://docs.rs/bevy_action_map).
//!
//! Use them like this:
//!
//! ```ignore
//! use bevy_action_map::prelude::*;
//!
//! #[derive(InputAction)]
//! #[action(path = "gameplay.jump", output = bool, intent = Button)]
//! struct Jump;
//!
//! #[derive(InputContext)]
//! #[context(path = "gameplay.on_foot", tick = Fixed)]
//! struct OnFoot;
//! ```
//!
//! Not compiled: what the macros expand to names `bevy_action_map`, and this crate cannot depend on
//! the one that re-exports it. The same example is checked for real in that crate's own docs and in
//! `tests/ui/pass`.

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
    category: Option<LitStr>,
    consume: bool,
}

struct ContextArgs {
    tick: Ident,
    priority: Option<LitInt>,
    path: LitStr,
    exclusive: bool,
}

fn expand_input_action(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    ensure_unit_struct(&input)?;
    let args = parse_action_args(&input.attrs, input.ident.span())?;
    let ident = input.ident;
    let path = args.path;

    let output = args.output;
    let intent = args.intent;
    let category = match args.category {
        Some(category) => quote!(::core::option::Option::Some(#category)),
        None => quote!(::core::option::Option::None),
    };
    let consume = args.consume;

    // Spelled out here rather than in the assertion, because a const assertion's message has to be
    // a literal — so the only chance to name the two halves of the mistake is at expansion time.
    let mismatch = format!(
        "`{}` declares `intent = {}`, which no `{}` action can serve. Either the intent or the \
         output shape is not the one you meant.",
        path.value(),
        intent,
        quote!(#output),
    );

    Ok(quote! {
        const _: () = ::core::assert!(
            ::bevy_action_map::action::Intent::#intent.is_one_of(
                <#output as ::bevy_action_map::action::ActionOutput>::INTENTS
            ),
            #mismatch
        );

        impl ::bevy_action_map::action::InputAction for #ident {
            type Output = #output;
            const INTENT: ::bevy_action_map::action::Intent = ::bevy_action_map::action::Intent::#intent;
            const PATH: &'static str = #path;
            const CATEGORY: ::core::option::Option<&'static str> = #category;
            const CONSUMES: bool = #consume;

            fn id() -> ::bevy_action_map::action::ActionId {
                // Rust has no generic statics, so the cache cannot live on the trait's default
                // method — but this impl is concrete, so it can hold one.
                static ID: ::bevy_action_map::action::ActionIdCache =
                    ::bevy_action_map::action::ActionIdCache::new();
                ID.get_or_intern::<Self>()
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
    let exclusive = args.exclusive;

    // A context is only usable as a component, so deriving one without the other is never what
    // was meant. `Default` and `Clone` come along because a scene format needs to construct and
    // copy the component to spawn it. All three are trivial on a unit struct; a context that needs
    // to configure its component differently should implement `InputContext` by hand, which is
    // four associated consts.
    Ok(quote! {
        impl ::bevy_action_map::action::InputContext for #ident {
            const TICK: ::bevy_action_map::action::TickDomain = ::bevy_action_map::action::TickDomain::#tick;
            const PRIORITY: i32 = #priority;
            const EXCLUSIVE: bool = #exclusive;
            const PATH: &'static str = #path;
        }

        impl ::bevy_action_map::__macro_exports::Component for #ident {
            const STORAGE_TYPE: ::bevy_action_map::__macro_exports::StorageType =
                ::bevy_action_map::__macro_exports::StorageType::Table;
            type Mutability = ::bevy_action_map::__macro_exports::Mutable;
        }

        impl ::core::default::Default for #ident {
            fn default() -> Self {
                Self
            }
        }

        impl ::core::clone::Clone for #ident {
            fn clone(&self) -> Self {
                Self
            }
        }

        impl ::core::marker::Copy for #ident {}
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
    let mut category = None;
    let mut consume = None;

    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("action"))
    {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("output") {
                set_once(&mut output, meta.value()?.parse::<Type>()?, &meta, "output")
            } else if meta.path.is_ident("intent") {
                set_once(
                    &mut intent,
                    meta.value()?.parse::<Ident>()?,
                    &meta,
                    "intent",
                )
            } else if meta.path.is_ident("path") {
                set_once(&mut path, meta.value()?.parse::<LitStr>()?, &meta, "path")
            } else if meta.path.is_ident("category") {
                set_once(
                    &mut category,
                    meta.value()?.parse::<LitStr>()?,
                    &meta,
                    "category",
                )
            } else if meta.path.is_ident("consume") {
                // A flag rather than `consume = true`: it reads as the thing it turns on, and
                // there is no `consume = false` to write because absent already means that.
                set_once(&mut consume, (), &meta, "consume")
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
        category,
        consume: consume.is_some(),
    })
}

/// Takes a value for a key, or refuses a key that was already given.
///
/// Without this the last one silently wins. That is bad for any of them and worst for `path`,
/// which is the key a player's saved bindings are stored against.
fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    meta: &syn::meta::ParseNestedMeta<'_>,
    key: &str,
) -> Result<()> {
    if slot.is_some() {
        return Err(meta.error(format!("`{key}` is given more than once")));
    }
    *slot = Some(value);
    Ok(())
}

fn parse_context_args(
    attributes: &[Attribute],
    missing_attr_span: proc_macro2::Span,
) -> Result<ContextArgs> {
    let mut tick = None;
    let mut priority = None;
    let mut path = None;
    let mut exclusive = None;

    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("context"))
    {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("tick") {
                set_once(&mut tick, meta.value()?.parse::<Ident>()?, &meta, "tick")
            } else if meta.path.is_ident("priority") {
                set_once(
                    &mut priority,
                    meta.value()?.parse::<LitInt>()?,
                    &meta,
                    "priority",
                )
            } else if meta.path.is_ident("path") {
                set_once(&mut path, meta.value()?.parse::<LitStr>()?, &meta, "path")
            } else if meta.path.is_ident("exclusive") {
                // A flag rather than `exclusive = true`, for the same reason `#[action(consume)]`
                // is one: it reads as the thing it turns on, and there is no `exclusive = false` to
                // write because absent already means that.
                set_once(&mut exclusive, (), &meta, "exclusive")
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
        exclusive: exclusive.is_some(),
    })
}
