use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::Parse, parse_macro_input, punctuated::Punctuated, 
    Expr, ExprLit, FnArg, Ident, Item, Lit, Pat, PatType, Token, Type, 
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

impl ToTokens for MatchPattern {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.pat.to_tokens(tokens)
    }
}

impl Parse for MatchPattern {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self { pat: syn::Pat::parse_single(input)? })
    }
}

struct CommandPatternArgs {
    data: (MatchPattern, Expr)
}

impl Into<(MatchPattern, Expr)> for CommandPatternArgs {
    fn into(self) -> (MatchPattern, Expr) {
        self.data
    }
}

impl Parse for CommandPatternArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut args = Punctuated::<Expr, Token![,]>::parse_terminated(input)?.into_iter();
        let first = syn::parse2(args.next().unwrap().into_token_stream())?;
        let second = syn::parse2(args.next().unwrap().into_token_stream())?;
        Ok(Self {
            data: (first, second)
        })
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
                match syn::parse2::<CommandPatternArgs>(list.tokens.clone()) {
                    Ok(args) => {
                        pattern = Some(args.data.0.into_token_stream());
                        error_lit = Some(args.data.1.into_token_stream());
                    },
                    Err(err) => {
                        pattern = Some(err.clone().into_compile_error());
                        error_lit = Some(err.into_compile_error());
                    }
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
    let new_ident = Ident::new(format!("__{}_generated", sig.ident).as_ref(), sig.ident.span());

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
        impl arke::server::command::CommandHandler for #ident {
            async fn handle(&mut self, command: arke::server::command::ArkeCommand) -> arke::server::command::ArkeCommand {
                #new_ident(&mut self.state, command).await
            }
        }
        
        #vis async fn #new_ident(#state_ident: &mut #state_type, #command_ident: arke::server::command::ArkeCommand) -> arke::server::command::ArkeCommand {
            match #command_ident {
                #pattern => {
                    let #state_ident = #state_ident;
                    #(#block)*
                },
                _ => #error_lit
            }
        }
    }
    .into()
}
