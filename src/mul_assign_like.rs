use quote::Tokens;
use syn::{parse_str, Data, DeriveInput, Ident, Fields, WhereClause};
use mul_like::{get_mul_generics, struct_exprs, tuple_exprs};
use std::iter;
use std::collections::HashSet;
use utils::{get_field_types_iter, unnamed_to_vec, named_to_vec};

pub fn expand(input: &DeriveInput, trait_name: &str) -> Tokens {
    let trait_ident = Ident::from(trait_name);
    let trait_path = &quote!(::std::ops::#trait_ident);
    let method_name = trait_name.to_string();
    let method_name = method_name.trim_right_matches("Assign");
    let method_name = method_name.to_lowercase();
    let method_ident = Ident::from(method_name.to_string() + "_assign");
    let input_type = &input.ident;
    let field_vec: &Vec<_>;

    let (exprs, fields) = match input.data {
        Data::Struct(data_struct) => match data_struct.fields {
            Fields::Unnamed(ref fields) => {
                field_vec = &unnamed_to_vec(fields);
                (tuple_exprs(field_vec, &method_ident), field_vec)
            },
            Fields::Named(ref fields) => {
                field_vec = &named_to_vec(fields);
                (struct_exprs(field_vec, &method_ident), field_vec)
            },
            _ => panic!(format!("Unit structs cannot use derive({})", trait_name)),
        }

        _ => panic!(format!("Only structs can use derive({})", trait_name)),
    };

    let scalar_ident = &Ident::from("__RhsT");
    let tys: &HashSet<_> = &get_field_types_iter(fields).collect();
    let scalar_iter = iter::repeat(scalar_ident);
    let trait_path_iter = iter::repeat(trait_path);

    let type_where_clauses: WhereClause = parse_str(&quote!{
        where #(#tys: #trait_path_iter<#scalar_iter>),*
    }.to_string()).unwrap();

    let new_generics = get_mul_generics(input, fields, scalar_ident, type_where_clauses);
    let (impl_generics, _, where_clause) = new_generics.split_for_impl();
    let (_, ty_generics, _) = input.generics.split_for_impl();

    quote!(
        impl#impl_generics #trait_path<#scalar_ident> for #input_type#ty_generics #where_clause{
            fn #method_ident(&mut self, rhs: #scalar_ident#ty_generics) {
                #(#exprs;
                  )*
            }
        }
    )
}
