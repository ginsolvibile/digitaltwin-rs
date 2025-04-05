use std::str::FromStr;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, AttributeArgs, Data, DeriveInput, Fields, ItemImpl, Lit, Meta, NestedMeta,
};

// ========== ACTOR ATTRIBUTE MACRO ==========

/// Main actor attribute macro. This transforms a regular struct into a state machine actor.
///
/// Example:
/// ```ignore
/// #[actor(default_state = "Off", slots("CurrentPowerDraw"))]
/// struct LightBulb {
///     #[actor_attr(default = "0.5")]
///     threshold: f32,
/// }
/// ```
#[proc_macro_attribute]
pub fn actor(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the attribute arguments
    let attr_args = parse_macro_input!(attr as AttributeArgs);

    // Parse the input struct
    let input = parse_macro_input!(item as DeriveInput);

    // Get the struct name and fields
    let name = &input.ident;
    let vis = &input.vis; // Preserve visibility
    let factory_name = format_ident!("{}Factory", name);

    // Extract default state from attributes
    let default_state = extract_default_state_from_attr_args(&attr_args)
        .unwrap_or_else(|| panic!("No default_state attribute found for Actor"));

    // Extract slots from attributes
    let slots = extract_slots_from_attr_args(&attr_args);

    // Extract fields and their default values
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields
                .named
                .iter()
                .map(|f| {
                    let name = &f.ident;
                    let ty = &f.ty;
                    let default = extract_default_value(&f.attrs);
                    (name, ty, default)
                })
                .collect::<Vec<_>>(),
            _ => panic!("Actor attribute only supports structs with named fields"),
        },
        _ => panic!("Actor attribute only supports structs"),
    };

    // Generate field declarations for the struct
    let field_decls: Vec<_> = fields
        .iter()
        .map(|(name, ty, _)| {
            quote! { #name: #ty, }
        })
        .collect();

    // For function parameters (no comma at the end)
    let fn_params: Vec<_> = fields
        .iter()
        .map(|(name, ty, _)| {
            quote! { #name: #ty }
        })
        .collect();

    let field_inits: Vec<_> = fields
        .iter()
        .map(|(name, _, _)| {
            quote! { #name, }
        })
        .collect();

    let field_copies: Vec<_> = fields
        .iter()
        .map(|(name, _, _)| {
            quote! { #name: self.#name, }
        })
        .collect();

    let default_values: Vec<_> = fields
        .iter()
        .map(|(_, _, default)| {
            quote! { #default }
        })
        .collect();

    let param_extractions: Vec<_> = fields
        .iter()
        .map(|(name, ty, default)| {
            quote! {
                let #name = params
                    .get(stringify!(#name))
                    .and_then(|v| v.as_f64())
                    .map(|v| v as #ty)
                    .unwrap_or(#default);
            }
        })
        .collect();

    // Generate slot literals for the slots method
    let slot_literals = slots.iter().map(|slot| {
        let slot_str = slot.as_str();
        quote! { #slot_str }
    });

    // Generate the implementation
    let output = quote! {
        use ::digitaltwin_core::StateBehavior;

        #[derive(Clone, Debug)]
        #vis struct #name<State> {
            // Actor-specific properties
            #(#field_decls)*
            // Generic actor properties
            dispatch_map: ::digitaltwin_core::DispatchMap<#name<State>>,
            command_map: ::digitaltwin_core::CommandMap<#name<State>>,
            _state: std::marker::PhantomData<State>,
        }

        impl<State> #name<State>
        where
            State: Send + Sync + 'static,
            #name<State>: ::digitaltwin_core::ActorState,
        {
            /// Create a new actor instance
            pub fn create(#(#fn_params),*) -> Box<::digitaltwin_core::ActorStateType> {
                Box::new(#name {
                    #(#field_inits)*
                    dispatch_map: <#default_state>::create_dispatch_map(),
                    command_map: <#default_state>::create_command_map(),
                    _state: std::marker::PhantomData::<_>,
                })
            }

            /// Define the actor's input slots
            pub fn slots() -> Vec<&'static str> {
                vec![#(#slot_literals),*]
            }

            /// Transition to another state
            fn transition<T>(&self) -> Box<::digitaltwin_core::ActorStateType>
            where
                #name<T>: ::digitaltwin_core::ActorState,
                T: ::digitaltwin_core::StateBehavior<Actor = #name<T>> + Send + Sync + 'static,
            {
                Box::new(#name {
                    #(#field_copies)*
                    dispatch_map: T::create_dispatch_map(),
                    command_map: T::create_command_map(),
                    _state: std::marker::PhantomData::<_>,
                })
            }
        }

        // ActorState implementation
        impl_actor_state!(#name);

        // Factory implementation
        #vis struct #factory_name;

        impl ::digitaltwin_core::ActorFactory for #factory_name {
            fn create_default() -> (Box<::digitaltwin_core::ActorStateType>, Vec<&'static str>) {
                (
                    #name::<#default_state>::create(#(#default_values),*),
                    #name::<#default_state>::slots(),
                )
            }

            fn create_with_params(params: serde_json::Value) -> (Box<::digitaltwin_core::ActorStateType>, Vec<&'static str>) {
                #(#param_extractions)*

                (
                    #name::<#default_state>::create(#(#field_inits)*),
                    #name::<#default_state>::slots(),
                )
            }
        }
    };

    TokenStream::from(output)
}

// ========== ACTOR STATE ATTRIBUTE MACRO ==========

/// Parsing struct for actor_state attribute
struct ActorStateArgs {
    actor: syn::Ident,
    state: syn::Ident,
}

impl Parse for ActorStateArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let actor = input.parse()?;
        input.parse::<syn::Token![,]>()?;
        let state = input.parse()?;
        Ok(ActorStateArgs { actor, state })
    }
}

/// The actor_state attribute macro. Adds state behavior implementation to an impl block.
///
/// Example:
/// ```ignore
/// #[actor_state(LightBulb, On)]
/// #[dispatch_map("CurrentPowerDraw" = power_change)]
/// #[command_map("SwitchOff" = switch_off)]
/// impl LightBulb<On> {
///    fn power_change(&self, pwr: f32) -> Box<ActorStateType> { ... }
///    fn switch_off(&self, _: serde_json::Value) -> Box<ActorStateType> { ... }
/// }
/// ```
#[proc_macro_attribute]
pub fn actor_state(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the attribute arguments
    let args = parse_macro_input!(attr as ActorStateArgs);

    // Use the parsed arguments
    let actor_ident = args.actor;
    let state_ident = args.state;

    // Parse the impl block
    let mut input = parse_macro_input!(item as ItemImpl);

    // Extract handler maps from attributes
    let (dispatch_entries, command_entries) = extract_handler_maps(&input);

    // Clean up attribute macros from the input
    input
        .attrs
        .retain(|attr| !attr.path.is_ident("dispatch_map") && !attr.path.is_ident("command_map"));

    // Generate dispatch map entries
    let dispatch_entries = dispatch_entries.iter().map(|(slot, handler)| {
        let slot_str = slot.as_str();
        quote! {
            map.insert(#slot_str, #actor_ident::<#state_ident>::#handler as fn(&Self::Actor, f32) -> Box<::digitaltwin_core::ActorStateType>);
        }
    });

    // Generate command map entries
    let command_entries = command_entries.iter().map(|(cmd, handler)| {
        let cmd_str = cmd.as_str();
        quote! {
            map.insert(#cmd_str, #actor_ident::<#state_ident>::#handler as fn(&Self::Actor, serde_json::Value) -> Box<::digitaltwin_core::ActorStateType>);
        }
    });

    // Generate state behavior implementation
    let output = quote! {
        #input

        impl ::digitaltwin_core::StateBehavior for #state_ident {
            type Actor = #actor_ident<#state_ident>;

            fn create_dispatch_map() -> ::digitaltwin_core::DispatchMap<Self::Actor> {
                let mut map = std::collections::HashMap::new();
                #(#dispatch_entries)*
                map
            }

            fn create_command_map() -> ::digitaltwin_core::CommandMap<Self::Actor> {
                let mut map = std::collections::HashMap::new();
                #(#command_entries)*
                map
            }

            fn state_name() -> String {
                stringify!(#state_ident).to_string()
            }
        }
    };

    TokenStream::from(output)
}

// ========== ACTOR STATE IMPLEMENTATION MACRO ==========
#[proc_macro]
pub fn impl_actor_state(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::Ident);

    let output = quote! {
        impl<S> ::digitaltwin_core::ActorState for #input<S>
        where
            S: ::digitaltwin_core::StateBehavior + Clone + Send + Sync + 'static,
        {
            fn input_change(&self, slot: &str, value: f32) -> Box<::digitaltwin_core::ActorStateType> {
                match self.dispatch_map.get(slot) {
                    Some(func) => func(self, value),
                    // TODO: notify error
                    None => Box::new((*self).clone()),
                }
            }

            fn execute(&self, command: &str, arg: ::serde_json::Value) -> Box<::digitaltwin_core::ActorStateType> {
                match self.command_map.get(command) {
                    Some(func) => func(self, arg),
                    // TODO: notify error
                    None => Box::new((*self).clone()),
                }
            }

            fn state(&self) -> String {
                S::state_name()
            }

            fn type_name(&self) -> String {
                stringify!(#input).to_string()
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
        }
    };

    TokenStream::from(output)
}

// ========== HELPER FUNCTIONS ==========

/// Extract the default state from attribute arguments
fn extract_default_state_from_attr_args(args: &[NestedMeta]) -> Option<syn::Ident> {
    for arg in args {
        if let NestedMeta::Meta(Meta::NameValue(name_value)) = arg {
            if name_value.path.is_ident("default_state") {
                if let Lit::Str(lit_str) = &name_value.lit {
                    return Some(syn::Ident::new(&lit_str.value(), Span::call_site()));
                }
            }
        }
    }
    None
}

/// Extract slots from attribute arguments
fn extract_slots_from_attr_args(args: &[NestedMeta]) -> Vec<String> {
    for arg in args {
        if let NestedMeta::Meta(Meta::List(list)) = arg {
            if list.path.is_ident("slots") {
                // Extract elements from the list
                let mut slots = Vec::new();
                for nested in &list.nested {
                    if let NestedMeta::Lit(Lit::Str(lit_str)) = nested {
                        slots.push(lit_str.value());
                    }
                }
                return slots;
            }
        }
    }
    Vec::new() // Empty slots if none provided
}

/// Extract default value from field attributes
fn extract_default_value(attrs: &[syn::Attribute]) -> proc_macro2::TokenStream {
    for attr in attrs {
        if attr.path.is_ident("actor_attr") {
            if let Ok(nested) = attr.parse_meta() {
                if let Meta::List(meta_list) = nested {
                    for nested_meta in meta_list.nested.iter() {
                        if let NestedMeta::Meta(Meta::NameValue(name_value)) = nested_meta {
                            if name_value.path.is_ident("default") {
                                if let Lit::Str(lit_str) = &name_value.lit {
                                    let tokens = lit_str.value();
                                    let literal = proc_macro2::TokenStream::from_str(&tokens)
                                        .expect("Invalid default value expression");
                                    return literal;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // If no default attribute found, return a compile error
    quote! {
        compile_error!("No default value provided for field. Use #[actor_attr(default = \"value\")]")
    }
}

/// Extract handler maps from attributed impl blocks
fn extract_handler_maps(item_impl: &ItemImpl) -> (Vec<(String, syn::Ident)>, Vec<(String, syn::Ident)>) {
    let mut dispatch_entries = Vec::new();
    let mut command_entries = Vec::new();

    for attr in &item_impl.attrs {
        if attr.path.is_ident("dispatch_map") || attr.path.is_ident("command_map") {
            let is_dispatch = attr.path.is_ident("dispatch_map");

            // Try parsing it as an attribute with tokens
            let attr_tokens = &attr.tokens;
            let attr_str = attr_tokens.to_string();

            // Manual parsing of the format: ("KeyName" = handler_name)
            if let Some(start_quote) = attr_str.find('"') {
                if let Some(end_quote) = attr_str[start_quote + 1..].find('"') {
                    let slot_or_cmd = attr_str[start_quote + 1..start_quote + 1 + end_quote].to_string();

                    if let Some(eq_pos) = attr_str[start_quote + 1 + end_quote..].find('=') {
                        let handler_start = start_quote + 1 + end_quote + eq_pos + 1;
                        if let Some(end_pos) = attr_str[handler_start..].find(')') {
                            let handler_name = attr_str[handler_start..handler_start + end_pos]
                                .trim()
                                .to_string();

                            let handler_ident =
                                syn::Ident::new(&handler_name, proc_macro2::Span::call_site());

                            if is_dispatch {
                                dispatch_entries.push((slot_or_cmd, handler_ident));
                            } else {
                                command_entries.push((slot_or_cmd, handler_ident));
                            }
                        }
                    }
                }
            }
        }
    }

    (dispatch_entries, command_entries)
}
