use crate::parser::transition::GuardExpression;
use crate::parser::AsyncIdent;
use syn::{parenthesized, parse, spanned::Spanned, token, Ident, Token, Type};

#[derive(Clone)]
pub struct Event {
    pub ident: Ident,
    pub data_type: Option<Type>,
}

pub struct EventMapping {
    pub in_state: Ident,
    pub event: Ident,
    pub transitions: Vec<Transition>,
}

pub struct Transition {
    pub guard: Option<GuardExpression>,
    pub action: Option<AsyncIdent>,
    pub out_state: Ident,
}

impl parse::Parse for Event {
    fn parse(input: parse::ParseStream) -> syn::Result<Self> {
        // Event
        input.parse::<Token![+]>()?;
        let ident: Ident = input.parse()?;

        // Possible type on the event
        let data_type = if input.peek(token::Paren) {
            let content;
            parenthesized!(content in input);
            let input: Type = content.parse()?;

            // Check so the type is supported
            match &input {
                Type::Array(_)
                | Type::Path(_)
                | Type::Ptr(_)
                | Type::Reference(_)
                | Type::Slice(_)
                | Type::Tuple(_) => (),
                _ => {
                    return Err(parse::Error::new(
                        input.span(),
                        "This is an unsupported type for events.",
                    ))
                }
            }

            Some(input)
        } else {
            None
        };

        Ok(Self { ident, data_type })
    }
}
