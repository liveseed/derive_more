use crate::utils::{SingleFieldData, State};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::Result, DeriveInput};

/// Provides the hook to expand `#[derive(Index)]` into an implementation of `From`
pub fn expand(input: &DeriveInput, trait_name: &'static str) -> Result<TokenStream> {
    let state = State::new(
        input,
        trait_name,
        quote!(::core::ops),
        String::from("deref_to_inner"),
    )?;
    let SingleFieldData {
        field_type,
        input_type,
        trait_path,
        impl_generics,
        ty_generics,
        where_clause,
        member,
        ..
    } = state.assert_single_enabled_field();

    Ok(quote! {
        impl#impl_generics #trait_path for #input_type#ty_generics #where_clause
        {
            type Target = #field_type;
            #[inline]
            fn deref(&self) -> &Self::Target {
                &#member
            }
        }
    })
}