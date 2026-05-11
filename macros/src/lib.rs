//! Derive macro for the `Message` trait.
//!
//! # Usage
//!
//! ```rust,ignore
//! use macros::Message;
//!
//! // Fire-and-forget message (Response = ())
//! #[derive(Message)]
//! struct Increment;
//!
//! // Message with a response
//! #[derive(Message)]
//! #[response(u64)]
//! struct GetCount;
//! ```

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_quote, DeriveInput, Type};

/// Derives the [`Message`] trait for a struct or enum.
///
/// The response type defaults to `()` and can be overridden with the
/// `#[response(Type)]` attribute.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Message)]
/// #[response(String)]
/// struct Greet(String);
/// ```
#[proc_macro_derive(Message, attributes(response))]
pub fn derive_message(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let response_type = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("response"))
        .map(|attr| {
            attr.parse_args::<Type>()
                .expect("Expected #[response(Type)]")
        })
        .unwrap_or_else(|| parse_quote!(()));

    quote! {
        impl Message for #name {
            type Response = #response_type;
        }
    }
    .into()
}
