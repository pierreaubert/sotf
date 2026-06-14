//! Test tagging proc-macros for the SOTF workspace.
//!
//! Each attribute expands to `#[ignore = "..."]` so that `cargo test` and
//! `cargo nextest` can selectively skip slow, network, or hardware-dependent
//! tests by default, while still allowing them to be run via explicit flags
//! or nextest profiles.

use proc_macro::TokenStream;
use quote::quote;

/// Mark a test as requiring network access.
#[proc_macro_attribute]
pub fn requires_network(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item: proc_macro2::TokenStream = item.into();
    quote! {
        #[ignore = "requires network access"]
        #item
    }
    .into()
}

/// Mark a test as slow / long-running.
#[proc_macro_attribute]
pub fn slow(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item: proc_macro2::TokenStream = item.into();
    quote! {
        #[ignore = "slow test"]
        #item
    }
    .into()
}

/// Mark a test as requiring real audio hardware.
#[proc_macro_attribute]
pub fn requires_hardware(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item: proc_macro2::TokenStream = item.into();
    quote! {
        #[ignore = "requires audio hardware"]
        #item
    }
    .into()
}
