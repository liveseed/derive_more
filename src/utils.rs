#![allow(dead_code)]

use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parse::{Error, Result},
    parse_str,
    spanned::Spanned,
    Attribute, Data, DeriveInput, Field, Fields, FieldsNamed, FieldsUnnamed, GenericParam,
    Generics, Ident, Index, Meta, NestedMeta, Type, TypeParamBound, WhereClause,
};

#[derive(Clone, Copy)]
pub enum RefType {
    No,
    Ref,
    Mut,
}

impl RefType {
    pub fn from_derive(trait_name: &str) -> (Self, &str) {
        if trait_name.ends_with("RefMut") {
            (RefType::Mut, trait_name.trim_end_matches("RefMut"))
        } else if trait_name.ends_with("Ref") {
            (RefType::Ref, trait_name.trim_end_matches("Ref"))
        } else {
            (RefType::No, trait_name)
        }
    }

    pub fn lifetime(self) -> TokenStream {
        match self {
            RefType::No => quote!(),
            _ => quote!('__deriveMoreLifetime),
        }
    }

    pub fn reference(self) -> TokenStream {
        match self {
            RefType::No => quote!(),
            RefType::Ref => quote!(&),
            RefType::Mut => quote!(&mut),
        }
    }

    pub fn mutability(self) -> TokenStream {
        match self {
            RefType::Mut => quote!(mut),
            _ => quote!(),
        }
    }

    pub fn pattern_ref(self) -> TokenStream {
        match self {
            RefType::Ref => quote!(ref),
            RefType::Mut => quote!(ref mut),
            _ => quote!(),
        }
    }

    pub fn reference_with_lifetime(self) -> TokenStream {
        if !self.is_ref() {
            return quote!();
        }
        let lifetime = self.lifetime();
        let mutability = self.mutability();
        quote!(&#lifetime #mutability)
    }

    pub fn is_ref(self) -> bool {
        match self {
            RefType::No => false,
            _ => true,
        }
    }
}

pub fn numbered_vars(count: usize, prefix: &str) -> Vec<Ident> {
    (0..count)
        .map(|i| Ident::new(&format!("__{}{}", prefix, i), Span::call_site()))
        .collect()
}

pub fn number_idents(count: usize) -> Vec<Index> {
    (0..count).map(Index::from).collect()
}

pub fn field_idents<'a>(fields: &'a [&'a Field]) -> Vec<&'a Ident> {
    fields
        .iter()
        .map(|f| {
            f.ident
                .as_ref()
                .expect("Tried to get field names of a tuple struct")
        })
        .collect()
}

pub fn get_field_types_iter<'a>(
    fields: &'a [&'a Field],
) -> Box<dyn Iterator<Item = &'a Type> + 'a> {
    Box::new(fields.iter().map(|f| &f.ty))
}

pub fn get_field_types<'a>(fields: &'a [&'a Field]) -> Vec<&'a Type> {
    get_field_types_iter(fields).collect()
}

pub fn add_extra_type_param_bound_op_output<'a>(
    generics: &'a Generics,
    trait_ident: &'a Ident,
) -> Generics {
    let mut generics = generics.clone();
    for type_param in &mut generics.type_params_mut() {
        let type_ident = &type_param.ident;
        let bound: TypeParamBound =
            parse_str(&quote!(::core::ops::#trait_ident<Output=#type_ident>).to_string()).unwrap();
        type_param.bounds.push(bound)
    }

    generics
}

pub fn add_extra_ty_param_bound_op<'a>(generics: &'a Generics, trait_ident: &'a Ident) -> Generics {
    add_extra_ty_param_bound(generics, &quote!(::core::ops::#trait_ident))
}

pub fn add_extra_ty_param_bound<'a>(generics: &'a Generics, bound: &'a TokenStream) -> Generics {
    let mut generics = generics.clone();
    let bound: TypeParamBound = parse_str(&bound.to_string()).unwrap();
    for type_param in &mut generics.type_params_mut() {
        type_param.bounds.push(bound.clone())
    }

    generics
}

pub fn add_extra_ty_param_bound_ref<'a>(
    generics: &'a Generics,
    bound: &'a TokenStream,
    ref_type: RefType,
) -> Generics {
    match ref_type {
        RefType::No => add_extra_ty_param_bound(generics, bound),
        _ => {
            let generics = generics.clone();
            let idents = generics.type_params().map(|x| &x.ident);
            let ref_with_lifetime = ref_type.reference_with_lifetime();
            add_extra_where_clauses(
                &generics,
                quote!(
                    where #(#ref_with_lifetime #idents: #bound),*
                ),
            )
        }
    }
}

pub fn add_extra_generic_param(generics: &Generics, generic_param: TokenStream) -> Generics {
    let generic_param: GenericParam = parse_str(&generic_param.to_string()).unwrap();
    let mut generics = generics.clone();
    generics.params.push(generic_param);

    generics
}

pub fn add_extra_where_clauses(generics: &Generics, type_where_clauses: TokenStream) -> Generics {
    let mut type_where_clauses: WhereClause = parse_str(&type_where_clauses.to_string()).unwrap();
    let mut new_generics = generics.clone();
    if let Some(old_where) = new_generics.where_clause {
        type_where_clauses.predicates.extend(old_where.predicates)
    }
    new_generics.where_clause = Some(type_where_clauses);

    new_generics
}

pub fn add_where_clauses_for_new_ident<'a>(
    generics: &'a Generics,
    fields: &[&'a Field],
    type_ident: &Ident,
    type_where_clauses: TokenStream,
) -> Generics {
    let generic_param = if fields.len() > 1 {
        quote!(#type_ident: ::core::marker::Copy)
    } else {
        quote!(#type_ident)
    };

    let generics = add_extra_where_clauses(generics, type_where_clauses);
    add_extra_generic_param(&generics, generic_param)
}

pub fn unnamed_to_vec(fields: &FieldsUnnamed) -> Vec<&Field> {
    fields.unnamed.iter().collect()
}

pub fn named_to_vec(fields: &FieldsNamed) -> Vec<&Field> {
    fields.named.iter().collect()
}

/// Checks whether `field` is decorated with the specifed simple attribute (e.g. `#[as_ref]`)
fn has_simple_attr(field: &Field, attr: &str) -> bool {
    field.attrs.iter().any(|a| {
        a.parse_meta()
            .map(|m| {
                m.path()
                    .segments
                    .first()
                    .map(|p| p.ident == attr)
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    })
}

/// Extracts types and identifiers from fields in the given struct
///
/// If `data` contains more than one field, only fields decorated with `attr` are considered.
pub fn extract_field_info<'a>(data: &'a Data, attr: &str) -> (Vec<&'a Type>, Vec<TokenStream>) {
    // Get iter over fields and check named/unnamed
    let named;
    let fields = match data {
        Data::Struct(data) => match data.fields {
            Fields::Named(_) => {
                named = true;
                data.fields.iter()
            }
            Fields::Unnamed(_) => {
                named = false;
                data.fields.iter()
            }
            Fields::Unit => panic!("struct must have one or more fields"),
        },
        _ => panic!("only structs may derive this trait"),
    };

    // If necessary, filter out undecorated fields
    let len = fields.len();
    let fields = fields.filter(|f| len == 1 || has_simple_attr(f, attr));

    // Extract info needed to generate impls
    if named {
        fields
            .map(|f| {
                let ident = f.ident.as_ref().unwrap();
                (&f.ty, quote!(#ident))
            })
            .unzip()
    } else {
        fields
            .enumerate()
            .map(|(i, f)| {
                let index = Index::from(i);
                (&f.ty, quote!(#index))
            })
            .unzip()
    }
}

fn panic_one_field(trait_name: &str, trait_attr: &str) -> ! {
    panic!(format!(
        "derive({}) only works when forwarding to a single field. Try putting #[{}] or #[{}(ignore)] on the fields in the struct",
        trait_name, trait_attr, trait_attr,
    ))
}

pub struct State<'a> {
    pub trait_name: &'static str,
    pub trait_module: TokenStream,
    pub trait_path: TokenStream,
    pub trait_attr: String,
    // input: &'a DeriveInput,
    pub named: bool,
    pub fields: Vec<&'a Field>,
    enabled: Vec<bool>,
}

impl<'a> State<'a> {
    pub fn new<'b>(
        input: &'b DeriveInput,
        trait_name: &'static str,
        trait_module: TokenStream,
        trait_attr: String,
    ) -> Result<State<'b>> {
        let trait_ident = Ident::new(trait_name, Span::call_site());
        let trait_path = quote!(#trait_module::#trait_ident);
        let named;
        let fields: Vec<_> = match input.data {
            Data::Struct(ref data_struct) => match data_struct.fields {
                Fields::Unnamed(ref fields) => {
                    named = false;
                    unnamed_to_vec(fields)
                }
                Fields::Named(ref fields) => {
                    named = true;
                    named_to_vec(fields)
                }
                Fields::Unit => {
                    named = false;
                    vec![]
                }
            },
            _ => panic_one_field(&trait_name, &trait_attr),
        };

        let ignore_attrs: Result<Vec<_>> = fields
            .iter()
            .map(|f| get_ignore_meta(&trait_attr, &f.attrs))
            .collect();
        let first_match = ignore_attrs?.into_iter().filter_map(|at| at).next();
        let enabled: Result<Vec<_>> = if let Some(first_match) = first_match {
            fields
                .iter()
                .map(|f| is_enabled(&trait_attr, &f.attrs, first_match))
                .collect()
        } else {
            Ok(vec![true; fields.len()])
        };

        Ok(State {
            trait_name,
            trait_module,
            trait_path,
            trait_attr,
            // input,
            fields,
            named,
            enabled: enabled?,
        })
    }

    pub fn assert_single_enabled_field(&self) -> (&'a Field, &'a Type, Box<dyn ToTokens>) {
        let enabled_fields = self.enabled_fields();
        if enabled_fields.len() != 1 {
            panic_one_field(self.trait_name, &self.trait_attr);
        };
        let mut field_idents = self.enabled_fields_idents();
        (
            enabled_fields[0],
            &enabled_fields[0].ty,
            field_idents.remove(0),
        )
    }

    fn enabled_fields(&self) -> Vec<&'a Field> {
        self.fields
            .iter()
            .zip(&self.enabled)
            .filter(|(_, ig)| **ig)
            .map(|(f, _)| *f)
            .collect()
    }

    fn field_idents(&self) -> Vec<Box<dyn ToTokens>> {
        if self.named {
            self.fields
                .iter()
                .map(|f| {
                    Box::new(
                        f.ident
                            .as_ref()
                            .expect("Tried to get field names of a tuple struct")
                            .clone(),
                    ) as Box<ToTokens>
                })
                .collect()
        } else {
            let count = self.fields.len();
            (0..count)
                .map(|i| Box::new(Index::from(i)) as Box<ToTokens>)
                .collect()
        }
    }

    fn enabled_fields_idents(&self) -> Vec<Box<dyn ToTokens>> {
        self.field_idents()
            .into_iter()
            .zip(&self.enabled)
            .filter(|(_, ig)| **ig)
            .map(|(f, _)| f)
            .collect()
    }
}

fn get_ignore_meta(trait_attr: &str, attrs: &[Attribute]) -> Result<Option<IgnoreMeta>> {
    let mut it = attrs
        .iter()
        .filter_map(|m| m.parse_meta().ok())
        .filter(|m| {
            if let Some(ident) = m.path().segments.first().map(|p| &p.ident) {
                ident == trait_attr
            } else {
                false
            }
        });

    let meta = if let Some(meta) = it.next() {
        meta
    } else {
        return Ok(None);
    };
    if let Some(meta2) = it.next() {
        return Err(Error::new(meta2.span(), "Too many formats given"));
    }
    let mut list = match meta.clone() {
        Meta::Path(_) => {
            return Ok(Some(IgnoreMeta::Enabled));
        }
        Meta::List(list) => list,
        _ => {
            return Err(Error::new(meta.span(), "Attribute format not supported1"));
        }
    };
    if list.nested.len() != 1 {
        return Err(Error::new(meta.span(), "Attribute format not supported2"));
    }
    let element = list.nested.pop().unwrap();
    let nested_meta = if let NestedMeta::Meta(meta) = element.value() {
        meta
    } else {
        return Err(Error::new(meta.span(), "Attribute format not supported3"));
    };
    if let Meta::Path(_) = nested_meta {
    } else {
        return Err(Error::new(meta.span(), "Attribute format not supported4"));
    }
    let ident = if let Some(ident) = nested_meta.path().segments.first().map(|p| &p.ident) {
        ident
    } else {
        return Err(Error::new(meta.span(), "Attribute format not supported5"));
    };
    if ident != "ignore" {
        return Err(Error::new(meta.span(), "Attribute format not supported6"));
    }
    Ok(Some(IgnoreMeta::Ignored))
}

fn is_enabled(trait_attr: &str, attrs: &[Attribute], first_match: IgnoreMeta) -> Result<bool> {
    let ignore_meta = if let Some(ignore_meta) = get_ignore_meta(trait_attr, attrs)? {
        ignore_meta
    } else {
        if first_match == IgnoreMeta::Enabled {
            return Ok(false);
        }
        return Ok(true);
    };
    Ok(ignore_meta == IgnoreMeta::Enabled)
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum IgnoreMeta {
    Enabled,
    Ignored,
}
