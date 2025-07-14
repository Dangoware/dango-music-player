#![allow(unused)]
use core::panic;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, Expr, Meta, MetaList, Path, Type, parenthesized, parse::Parse, token::Token};

#[proc_macro_attribute]
pub fn command(_: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::Item);
    let command_input_num: usize = 0;

    if let syn::Item::Enum(ref item) = input {
        let ident = &item.ident;
        let variants = &item.variants;

        let response_ident = format_ident!("{}Response", ident);
        let response_functions = variants
            .iter()
            .map(|variant| {
                // this assumes that the ordering of the attributes is #[response(Foo)], then #[function(bar)]
                let variant_attrs = &variant.attrs;
                let response = &variant_attrs[0];
                let function = &variant_attrs[1];

                // that can't be efficient
                let variant_inputs = variant
                    .fields
                    .iter()
                    .map(|field| &field.ty)
                    .collect::<Vec<_>>();

                let response_path = if response.path().is_ident("response") {
                    if let Type::Path(p) = response.parse_args().expect("failed to parse response")
                    {
                        p.path
                    } else {
                        panic!("should have been a type path stupid");
                    }
                } else {
                    panic!("ident isn't response")
                };

                let function_path = if function.path().is_ident("function") {
                    let parsed = function.parse_args().expect("failed to parse function");
                    if let Type::Path(function_sign) = parsed {
                        function_sign
                    } else {
                        panic!("That should be a function {:?}", parsed)
                    }
                } else {
                    panic!("There should be a function to go with the response")
                };

                Command {
                    // fields: todo!(),
                    return_type: response_path,
                    function: function_path.path,
                }
            })
            .collect::<Vec<_>>();

        let response_types = response_functions
            .iter()
            .map(|command| quote! { #response });
        let function_paths = response_functions
            .iter()
            .map(|Command| quote! { #function });

        let response_variants = &item.variants.iter().enumerate().map(|(i, field)| {});

        // fn attribute needs to check for if the response tokens are the same types as the input fn

        quote! {
            #input

            // #(#response_types)*

            // #(#function_paths)*
        }
        .into()
    } else {
        panic!("needs to be enum")
    }
}

struct Command {
    // fields: Vec<Path>,
    return_type: Path,
    function: Path,
}
