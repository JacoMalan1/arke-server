use std::str::FromStr;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::Parse, parse_macro_input, punctuated::Punctuated, DeriveInput, Expr, ExprLit, FnArg,
    GenericArgument, Ident, Item, Lit, MetaNameValue, Pat, PatType, Path, PathArguments,
    ReturnType, Token, Type, TypePath, ExprStruct,
};

struct ConversationHandlerInput {
    args: Punctuated<syn::Meta, Token![,]>,
}

impl From<Punctuated<syn::Meta, Token![,]>> for ConversationHandlerInput {
    fn from(value: Punctuated<syn::Meta, Token![,]>) -> Self {
        Self { args: value }
    }
}

impl Parse for ConversationHandlerInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Punctuated::<syn::Meta, Token![,]>::parse_terminated(input)?.into())
    }
}

struct MatchPattern {
    pat: syn::Pat
}

impl Parse for MatchPattern {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self { pat: syn::Pat::parse_single(input)? })
    }
}

enum CommandPatternArg {
    MatchPattern(MatchPattern),
    ErrorExpression(Expr)
}

impl Parse for CommandPatternArg {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if let Some(pat) = MatchPattern::parse(input).ok() {
            Ok(CommandPatternArg::MatchPattern(pat))
        } else {
            Ok(CommandPatternArg::ErrorExpression(Expr::parse(input)?))
        }
    }
}

struct CommandPatternArgs {
    args: Punctuated<CommandPatternArg, Token![,]>
}

impl Parse for CommandPatternArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self { args: Punctuated::<CommandPatternArg, Token![,]>::parse_terminated(input)? })
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

    let (_, error_type) = if let ReturnType::Type(_, ty) = sig.output {
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
    let mut error_lit = None;
    let mut pattern = None;
    args.into_iter().for_each(|arg| {
        if let syn::Meta::NameValue(name_value) = arg.clone() {
            if name_value.path.segments.last().unwrap().ident == "state" {
                state_ident = Some(
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(val), ..
                    }) = name_value.value.clone()
                    {
                        Ident::new(val.value().as_ref(), val.span())
                    } else {
                        panic!("Expected string literal argument for 'state'");
                    },
                )
            }
        }
        if let syn::Meta::List(list) = arg {
            if list.path.segments.last().unwrap().ident == "command" {
                let args = syn::parse2::<CommandPatternArgs>(list.tokens).unwrap().args;
                if args.len() != 2 {
                    panic!("Expected two arguments for `command`");
                }

                let first = args.first().unwrap();
                let second = args.last().unwrap();

                if let CommandPatternArg::MatchPattern(pat) = first {
                    pattern = Some(pat.pat.clone())
                } else {
                    panic!("First argument must be a pattern");
                }

                if let CommandPatternArg::ErrorExpression(expr) = second {
                    error_lit = Some(expr.clone())
                } else {
                    panic!("Second argument must be an expression");
                }
            }
        }
    });
    let mut state_ident = state_ident.expect("Missing 'state' argument");
    let pattern = pattern.expect("Missing 'pattern' argument");
    let error_lit = error_lit.expect("Missing command mismatch error");

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

    let block = block.stmts;
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
            async fn handle(&mut self, command: crate::server::ArkeCommand) -> Result<crate::server::ArkeCommand, Self::Error> {
                Ok(#new_ident(&mut self.state, command).await?)
            }
        }
        
        #vis async fn #new_ident(#state_ident: &mut #state_type, #command_ident: crate::server::ArkeCommand) -> Result<crate::server::ArkeCommand, #error_type> {
            match #command_ident {
                #pattern => {
                    let #state_ident = #state_ident;
                    #(#block)*
                },
                _ => Err(#error_lit)
            }
        }
    }
    .into()
}
