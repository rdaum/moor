// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! Implementation of enum conversion derive macro.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Attribute, Data, DeriveInput, Error, Fields, Ident, Meta, Path, Result, Token,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

/// Extract the FlatBuffer type path from `#[flatbuffer(path::to::Type)]`
fn get_flatbuffer_type(attrs: &[Attribute]) -> Result<Path> {
    for attr in attrs {
        if attr.path().is_ident("flatbuffer") {
            let path: Path = attr.parse_args()?;
            return Ok(path);
        }
    }
    Err(Error::new(
        proc_macro2::Span::call_site(),
        "missing #[flatbuffer(Type)] attribute",
    ))
}

/// Variant mapping info extracted from attributes
struct VariantMapping {
    rust_name: Ident,
    fb_name: Option<Ident>,
    skip: bool,
    #[allow(dead_code)] // May be used for validation in the future
    has_fields: bool,
}

/// Extract variant mapping from `#[fb(Name)]` or `#[fb(skip)]`
fn get_variant_mapping(
    variant_name: &Ident,
    attrs: &[Attribute],
    fields: &Fields,
) -> Result<VariantMapping> {
    let has_fields = !matches!(fields, Fields::Unit);
    let mut fb_name = None;
    let mut skip = false;

    for attr in attrs {
        if !attr.path().is_ident("fb") {
            continue;
        }

        let meta = &attr.meta;
        match meta {
            Meta::List(list) => {
                let tokens = list.tokens.clone();
                let token_str = tokens.to_string();

                if token_str == "skip" {
                    skip = true;
                } else {
                    // Parse as identifier
                    let ident: Ident = syn::parse2(tokens)?;
                    fb_name = Some(ident);
                }
            }
            _ => {
                return Err(Error::new(
                    attr.span(),
                    "expected #[fb(VariantName)] or #[fb(skip)]",
                ));
            }
        }
    }

    // If no explicit mapping and not skipped, error for variants with fields
    if fb_name.is_none() && !skip && has_fields {
        return Err(Error::new(
            variant_name.span(),
            format!(
                "variant `{}` has fields and must either have #[fb(Name)] mapping or #[fb(skip)]",
                variant_name
            ),
        ));
    }

    // If no explicit mapping and not skipped, use same name
    if fb_name.is_none() && !skip {
        fb_name = Some(variant_name.clone());
    }

    Ok(VariantMapping {
        rust_name: variant_name.clone(),
        fb_name,
        skip,
        has_fields,
    })
}

pub fn derive_enum_flatbuffer_impl(input: DeriveInput) -> Result<TokenStream> {
    let enum_name = &input.ident;
    let fb_type = get_flatbuffer_type(&input.attrs)?;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => {
            return Err(Error::new(
                input.span(),
                "EnumFlatbuffer can only be derived for enums",
            ));
        }
    };

    let mut mappings = Vec::new();
    let mut skipped = Vec::new();

    for variant in variants {
        let mapping = get_variant_mapping(&variant.ident, &variant.attrs, &variant.fields)?;
        if mapping.skip {
            skipped.push(mapping);
        } else {
            mappings.push(mapping);
        }
    }

    // Generate From<Self> for FbType
    let to_fb_arms: Vec<_> = mappings
        .iter()
        .map(|m| {
            let rust_name = &m.rust_name;
            let fb_name = m.fb_name.as_ref().unwrap();
            quote! {
                #enum_name::#rust_name => #fb_type::#fb_name
            }
        })
        .collect();

    // Generate From<FbType> for Self
    let from_fb_arms: Vec<_> = mappings
        .iter()
        .map(|m| {
            let rust_name = &m.rust_name;
            let fb_name = m.fb_name.as_ref().unwrap();
            quote! {
                #fb_type::#fb_name => #enum_name::#rust_name
            }
        })
        .collect();

    // If we have skipped variants, we need to generate TryFrom instead of From
    // for the FbType -> Self direction, and panic/unreachable for Self -> FbType
    let (to_fb_impl, from_fb_impl) = if skipped.is_empty() {
        // Simple case: all variants mapped
        let to_fb = quote! {
            impl ::core::convert::From<#enum_name> for #fb_type {
                fn from(e: #enum_name) -> #fb_type {
                    match e {
                        #(#to_fb_arms),*
                    }
                }
            }
        };
        let from_fb = quote! {
            impl ::core::convert::From<#fb_type> for #enum_name {
                fn from(e: #fb_type) -> #enum_name {
                    match e {
                        #(#from_fb_arms),*
                    }
                }
            }
        };
        (to_fb, from_fb)
    } else {
        // Complex case: some variants skipped
        let skipped_names: Vec<_> = skipped.iter().map(|m| &m.rust_name).collect();
        let to_fb = quote! {
            impl #enum_name {
                /// Convert to FlatBuffer type. Panics for skipped variants.
                pub fn to_flatbuffer(&self) -> #fb_type {
                    match self {
                        #(#to_fb_arms),*,
                        #(#enum_name::#skipped_names { .. } => {
                            panic!(concat!(
                                "cannot convert ",
                                stringify!(#enum_name),
                                "::",
                                stringify!(#skipped_names),
                                " to FlatBuffer automatically"
                            ))
                        }),*
                    }
                }
            }
        };
        let from_fb = quote! {
            impl ::core::convert::From<#fb_type> for #enum_name {
                fn from(e: #fb_type) -> #enum_name {
                    match e {
                        #(#from_fb_arms),*
                    }
                }
            }
        };
        (to_fb, from_fb)
    };

    Ok(quote! {
        #to_fb_impl
        #from_fb_impl
    })
}

// ============================================================================
// Declarative macro implementation
// ============================================================================

struct EnumMappingInput {
    rust_type: Path,
    fb_type: Path,
    mappings: Vec<(Ident, Ident)>,
}

impl Parse for EnumMappingInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let rust_type: Path = input.parse()?;
        input.parse::<Token![<]>()?;
        input.parse::<Token![=]>()?;
        input.parse::<Token![>]>()?;
        let fb_type: Path = input.parse()?;

        let content;
        syn::braced!(content in input);

        let mut mappings = Vec::new();
        while !content.is_empty() {
            let rust_variant: Ident = content.parse()?;
            content.parse::<Token![<]>()?;
            content.parse::<Token![=]>()?;
            content.parse::<Token![>]>()?;
            let fb_variant: Ident = content.parse()?;

            mappings.push((rust_variant, fb_variant));

            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(EnumMappingInput {
            rust_type,
            fb_type,
            mappings,
        })
    }
}

pub fn define_enum_mapping_impl(input: TokenStream) -> Result<TokenStream> {
    let EnumMappingInput {
        rust_type,
        fb_type,
        mappings,
    } = syn::parse2(input)?;

    let to_fb_arms: Vec<_> = mappings
        .iter()
        .map(|(rust, fb)| {
            quote! {
                #rust_type::#rust => #fb_type::#fb
            }
        })
        .collect();

    let from_fb_arms: Vec<_> = mappings
        .iter()
        .map(|(rust, fb)| {
            quote! {
                #fb_type::#fb => #rust_type::#rust
            }
        })
        .collect();

    // Clone the arms for reference implementations
    let to_fb_arms_ref = to_fb_arms.clone();
    let from_fb_arms_ref = from_fb_arms.clone();

    Ok(quote! {
        // Owned conversions
        impl ::core::convert::From<#rust_type> for #fb_type {
            fn from(e: #rust_type) -> #fb_type {
                match e {
                    #(#to_fb_arms),*
                }
            }
        }

        impl ::core::convert::From<#fb_type> for #rust_type {
            fn from(e: #fb_type) -> #rust_type {
                match e {
                    #(#from_fb_arms),*
                }
            }
        }

        // Reference conversions (for ergonomic use with borrowed values)
        impl ::core::convert::From<&#rust_type> for #fb_type {
            fn from(e: &#rust_type) -> #fb_type {
                match e {
                    #(#to_fb_arms_ref),*
                }
            }
        }

        impl ::core::convert::From<&#fb_type> for #rust_type {
            fn from(e: &#fb_type) -> #rust_type {
                match e {
                    #(#from_fb_arms_ref),*
                }
            }
        }
    })
}
