use std::collections::HashMap;

use quote::Tokens;
use syn::{Body, Ident, Variant, VariantData, MacroInput, Ty};


/// Provides the hook to expand `#[derive(From)]` into an implementation of `From`
pub fn expand(input: &MacroInput, _: &str) -> Tokens {
    let name = input.ident.clone();
    match input.body {
        Body::Struct(VariantData::Tuple(ref fields)) => {
            if fields.len() == 1 {
                newtype_from(name, fields[0].ty.clone())
            }
            else {
                panic!("Only Tuple structs with a single field can derive From")
            }
        }
        Body::Enum(ref variants) => {
            enum_from(name, variants)
        }
        _ => panic!("Only newtype structs can derive From")
    }
}


fn newtype_from(new_type: Ident, old_type: Ty) -> Tokens {
    quote!{
        impl ::std::convert::From<#old_type> for #new_type {
            fn from(a: #old_type) -> #new_type {
                #new_type(a)
            }
        }
    }
}

fn enum_from(enum_ident: Ident, variants: &Vec<Variant>) -> Tokens {
    let mut types = vec![];
    let mut idents = vec![];
    let mut type_counts = HashMap::new();

    for variant in variants {
        match variant.data {
            VariantData::Tuple(ref structs) => {
                if structs.len() == 1 {
                    let ty = structs[0].ty.clone();
                    idents.push(variant.ident.clone());
                    types.push(ty.clone());
                    let counter = type_counts.entry(ty).or_insert(0);
                    *counter += 1;
                }
            }
            _ => {},
        }
    }

    let mut tokens = Tokens::new();

    for (ident, old_type) in idents.iter().zip(types) {
        if *type_counts.get(&old_type).unwrap() != 1 {
            // If more than one newtype is present don't add automatic From, since it is
            // ambiguous.
            continue
        }

        tokens.append(&quote!(
            impl ::std::convert::From<#old_type> for #enum_ident {
                fn from(a: #old_type) -> #enum_ident {
                    #enum_ident::#ident(a)
                }
            }
        ).to_string())
    }
    tokens
}



