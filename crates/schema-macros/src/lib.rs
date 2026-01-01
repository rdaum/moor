// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Proc macros for reducing FlatBuffer conversion boilerplate.
//!
//! Provides derive macros for generating bidirectional enum conversions between
//! Rust domain types and FlatBuffer schema types.

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod enum_convert;

/// Derive macro for bidirectional enum conversion with FlatBuffer types.
///
/// Generates `From` implementations in both directions for simple enums
/// (unit variants only). For enums with data-carrying variants, use
/// `#[fb(skip)]` to exclude them from auto-generation.
///
/// # Attributes
///
/// ## Container attributes (on the enum)
/// - `#[flatbuffer(path::to::FbEnum)]` - Required. The FlatBuffer enum type.
///
/// ## Variant attributes
/// - `#[fb(VariantName)]` - Map to FlatBuffer variant with this name.
/// - `#[fb(skip)]` - Skip this variant (must be handled manually).
///
/// # Example
///
/// ```ignore
/// use moor_schema_macros::EnumFlatbuffer;
///
/// #[derive(EnumFlatbuffer)]
/// #[flatbuffer(common::ArgSpec)]
/// pub enum ArgSpec {
///     #[fb(None)]
///     None,
///     #[fb(Any)]
///     Any,
///     #[fb(This)]
///     This,
/// }
/// ```
///
/// This generates:
/// ```ignore
/// impl From<ArgSpec> for common::ArgSpec {
///     fn from(e: ArgSpec) -> common::ArgSpec {
///         match e {
///             ArgSpec::None => common::ArgSpec::None,
///             ArgSpec::Any => common::ArgSpec::Any,
///             ArgSpec::This => common::ArgSpec::This,
///         }
///     }
/// }
///
/// impl From<common::ArgSpec> for ArgSpec {
///     fn from(e: common::ArgSpec) -> ArgSpec {
///         match e {
///             common::ArgSpec::None => ArgSpec::None,
///             common::ArgSpec::Any => ArgSpec::Any,
///             common::ArgSpec::This => ArgSpec::This,
///         }
///     }
/// }
/// ```
#[proc_macro_derive(EnumFlatbuffer, attributes(flatbuffer, fb))]
pub fn derive_enum_flatbuffer(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    enum_convert::derive_enum_flatbuffer_impl(input)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// Declarative macro for defining enum mappings inline.
///
/// Alternative to the derive macro when you need more control or when
/// working with enums you don't own.
///
/// # Example
///
/// ```ignore
/// define_enum_mapping! {
///     MyEnum <=> fb::FbEnum {
///         VariantA <=> FbVariantA,
///         VariantB <=> FbVariantB,
///     }
/// }
/// ```
#[proc_macro]
pub fn define_enum_mapping(input: TokenStream) -> TokenStream {
    enum_convert::define_enum_mapping_impl(input.into())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}
