use crate::utils::{
    add_extra_generic_param, numbered_vars, DeriveType, RefType, State,
};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use std::collections::HashMap;
use syn::{DeriveInput, Result};

/// Provides the hook to expand `#[derive(TryInto)]` into an implementation of `TryInto`
#[allow(clippy::cognitive_complexity)]
pub fn expand(input: &DeriveInput, trait_name: &'static str) -> Result<TokenStream> {
    let (ref_type, _) = RefType::from_derive(trait_name);

    let state = State::new(
        input,
        trait_name,
        quote!(::core::convert),
        String::from("try_into") + ref_type.attr_suffix(),
    )?;
    if state.derive_type != DeriveType::Enum {
        panic!("Only enums can derive TryInto");
    }

    let mut variants_per_types = HashMap::new();
    let pattern_ref = ref_type.pattern_ref();
    let lifetime = ref_type.lifetime();
    let reference_with_lifetime = ref_type.reference_with_lifetime();

    let generics_impl;
    let (_, ty_generics, where_clause) = input.generics.split_for_impl();
    let (impl_generics, _, _) = if ref_type.is_ref() {
        generics_impl = add_extra_generic_param(&input.generics, lifetime.clone());
        generics_impl.split_for_impl()
    } else {
        input.generics.split_for_impl()
    };
    let input_type = &input.ident;

    for variant_state in state.enabled_variant_data().variant_states {
        let multi_field_data = variant_state.enabled_fields_data();
        variants_per_types
            .entry(multi_field_data.field_types.clone())
            .or_insert_with(Vec::new)
            .push(multi_field_data);
    }

    let mut tokens = TokenStream::new();

    for (ref original_types, ref multi_field_datas) in variants_per_types {
        let mut matchers = vec![];
        let vars = &numbered_vars(original_types.len(), "");
        for multi_field_data in multi_field_datas {
            let patterns: Vec<_> = vars.iter().map(|var| quote!(#pattern_ref #var)).collect();
            matchers.push(multi_field_data.initializer(&patterns));
        }

        let vars = if vars.len() == 1 {
            quote!(#(#vars)*)
        } else {
            quote!((#(#vars),*))
        };

        let output_type = if original_types.len() == 1 {
            format!("{}", quote!(#(#original_types)*))
        } else {
            let types = original_types
                .iter()
                .map(|t| format!("{}", quote!(#t)))
                .collect::<Vec<_>>();
            format!("({})", types.join(", "))
        };
        let variant_names = multi_field_datas
            .iter()
            .map(|d| {
                format!(
                    "{}",
                    d.variant_name.expect("Somehow there was no variant name")
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let message =
            format!("Only {} can be converted to {}", variant_names, output_type);

        let try_from = quote! {
            impl#impl_generics ::core::convert::TryFrom<#reference_with_lifetime #input_type#ty_generics> for
                (#(#reference_with_lifetime #original_types),*) #where_clause {
                type Error = &'static str;

                #[allow(unused_variables)]
                #[inline]
                fn try_from(value: #reference_with_lifetime #input_type#ty_generics) -> ::core::result::Result<Self, Self::Error> {
                    match value {
                        #(#matchers)|* => ::core::result::Result::Ok(#vars),
                        _ => ::core::result::Result::Err(#message),
                    }
                }
            }
        };
        try_from.to_tokens(&mut tokens)
    }
    Ok(tokens)
}
