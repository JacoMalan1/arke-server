#![feature(trace_macros)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::Parse, parse_macro_input, punctuated::Punctuated, DeriveInput, Expr, ExprLit, FnArg,
    GenericArgument, Ident, Item, Lit, MetaNameValue, Pat, PatType, Path, PathArguments,
    ReturnType, Token, Type, TypePath,
};

struct ConversationHandlerInput {
    args: Punctuated<MetaNameValue, Token![,]>,
}

impl From<Punctuated<MetaNameValue, Token![,]>> for ConversationHandlerInput {
    fn from(value: Punctuated<MetaNameValue, Token![,]>) -> Self {
        Self { args: value }
    }
}

impl Parse for ConversationHandlerInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Punctuated::<MetaNameValue, Token![,]>::parse_terminated(input)?.into())
    }
}

#[proc_macro_attribute]
pub fn conversation_handler(args: TokenStream, annotated_item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ConversationHandlerInput).args;
    let annotated_item = parse_macro_input!(annotated_item as Item);

    let (vis, ident, block, sig) = if let Item::Fn(fun) = annotated_item {
        (fun.vis, fun.sig.ident.clone(), fun.block, fun.sig)
    } else {
        panic!("Macro can only be used to annotate function items")
    };

    let (return_type, error_type) = if let ReturnType::Type(_, ty) = sig.output {
        if let Type::Path(TypePath {
            path: Path { segments, .. },
            ..
        }) = *ty.clone()
        {
            if segments.last().unwrap().ident != "Result" {
                panic!("Must return a result");
            }
            let last_segment = segments.last().unwrap().arguments.clone();
            if let PathArguments::AngleBracketed(path_args) = last_segment {
                let return_type = if let GenericArgument::Type(ty) = path_args.args.first().unwrap()
                {
                    ty.clone()
                } else {
                    panic!("Must have type as first generic argument");
                };
                let error_type = if let GenericArgument::Type(ty) = path_args.args.last().unwrap() {
                    ty.clone()
                } else {
                    panic!("Must have type as last generic argument");
                };

                (return_type, error_type)
            } else {
                panic!("Must have angle-bracketed args");
            }
        } else {
            panic!("Return type must be a path")
        }
    } else {
        panic!("Function must have a return type")
    };

    let mut state_ident = None;
    args.into_iter().for_each(|arg| {
        if arg.path.segments.last().unwrap().ident == "state" {
            state_ident = Some(
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(val), ..
                }) = arg.value
                {
                    Ident::new(val.value().as_ref(), val.span())
                } else {
                    panic!("Expected string literal argument for 'state'");
                },
            )
        }
    });
    let mut state_ident = state_ident.expect("Missing 'state' argument");

    let mut state_type = None;
    let mut command_ident = None;
    sig.inputs.clone().into_iter().for_each(|input| {
        if let FnArg::Typed(PatType { pat, ty, .. }) = input {
            if let Pat::Ident(ident) = *pat {
                if let Type::Path(path) = *ty {
                    if path.path.segments.last().unwrap().ident == "ArkeCommand" {
                        command_ident = Some(ident.ident);
                    }
                }
            }
        }
    });
    let command_ident = command_ident.expect("Function must have an input variable `command`");
    
    sig.inputs.into_iter().for_each(|input| {
        if let FnArg::Typed(PatType { pat, ty, .. }) = input {
            if let Pat::Ident(ident) = *pat {
                if ident.ident == state_ident {
                    state_ident = ident.ident.clone();
                    state_type = Some(*ty);
                }
            }
        }
    });
    let state_type = state_type.expect("Couldn't determine state type");
    let new_ident = Ident::new(format!("{}_generated", sig.ident).as_ref(), sig.ident.span());

    quote! {
        #[allow(non_camel_case_types)]
        #vis struct #ident {
            state: #state_type
        }

        impl #ident {
            pub fn new(state: #state_type) -> Self {
                Self {
                    state: state
                }
            }
        }

        #[async_trait::async_trait]
        impl crate::server::ConversationHandler<#state_type> for #ident {
            type Error = #error_type;
            type Output = #state_type;
            async fn handle(self, command: crate::server::ArkeCommand) -> Result<Self::Output, Self::Error> {
                Ok(#new_ident(self.state, command).await?)
            }

        }
        
        #vis async fn #new_ident(#state_ident: #state_type, #command_ident: ArkeCommand) -> Result<#state_type, #error_type> #block
    }
    .into()
}
