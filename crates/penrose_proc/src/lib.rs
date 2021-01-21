//! Proc macros for use in the main Penrose crate
use proc_macro::TokenStream;
use syn::{
    parenthesized,
    parse::{Parse, ParseStream, Result},
    parse_macro_input,
    punctuated::Punctuated,
    LitStr, Token,
};

use std::{collections::HashSet, process::Command};

const VALID_MODIFIERS: [&str; 4] = ["A", "M", "S", "C"];

struct Binding {
    raw: String,
    mods: Vec<String>,
    keyname: Option<String>,
}

struct BindingsInput(Vec<Binding>);

impl Parse for BindingsInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut bindings = as_bindings(comma_sep_strs(input)?);

        let templated_content;
        parenthesized!(templated_content in input);

        while !templated_content.is_empty() {
            let content;
            parenthesized!(content in templated_content);
            bindings.extend(expand_templates(
                comma_sep_strs(&content)?,
                comma_sep_strs(&content)?,
            ));
        }

        Ok(Self(bindings))
    }
}

fn comma_sep_strs(input: ParseStream) -> Result<Vec<String>> {
    let content;
    parenthesized!(content in input);
    Ok(Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?
        .iter()
        .map(LitStr::value)
        .collect())
}

fn as_bindings(raw: Vec<String>) -> Vec<Binding> {
    raw.iter()
        .map(|s| {
            let mut parts: Vec<&str> = s.split('-').collect();
            let (keyname, mods) = if parts.len() <= 1 {
                (None, vec![s.clone()])
            } else {
                (
                    parts.pop().map(String::from),
                    parts.into_iter().map(String::from).collect(),
                )
            };

            Binding {
                raw: s.clone(),
                keyname,
                mods,
            }
        })
        .collect()
}

fn expand_templates(templates: Vec<String>, keynames: Vec<String>) -> Vec<Binding> {
    templates
        .iter()
        .flat_map(|t| {
            let mut parts: Vec<&str> = t.split('-').collect();
            if parts.pop() != Some("{}") {
                panic!(
                    "'{}' is an invalid template: expected '<Modifiers>-{{}}'",
                    t
                )
            };
            keynames
                .iter()
                .map(|k| Binding {
                    raw: format!("{}-{}", parts.join("-"), k),
                    mods: parts.iter().map(|m| m.to_string()).collect(),
                    keyname: Some(k.into()),
                })
                .collect::<Vec<Binding>>()
        })
        .collect()
}

fn keynames_from_xmodmap() -> Vec<String> {
    let res = Command::new("xmodmap")
        .arg("-pke")
        .output()
        .expect("unable to fetch keycodes via xmodmap: please ensure that it is installed");

    // each line should match 'keycode <code> = <names ...>'
    String::from_utf8(res.stdout)
        .expect("received invalid utf8 from xmodmap")
        .lines()
        .flat_map(|s| s.split_whitespace().skip(3).map(|name| name.into()))
        .collect()
}

fn has_valid_modifiers(binding: &Binding) -> bool {
    !binding.mods.is_empty()
        && binding
            .mods
            .iter()
            .all(|s| VALID_MODIFIERS.contains(&s.as_ref()))
}

fn is_valid_keyname(binding: &Binding, names: &[String]) -> bool {
    if let Some(ref k) = binding.keyname {
        names.contains(&k)
    } else {
        false
    }
}

fn report_error(msg: impl AsRef<str>, b: &Binding) {
    panic!(
        "'{}' is an invalid key binding: {}\n\
        Key bindings should be of the form <modifiers>-<key name> e.g:  M-j, M-S-slash, M-C-Up",
        b.raw,
        msg.as_ref()
    )
}

/// This is an internal macro that is used as part of `gen_keybindings` to validate user provided
/// key bindings at compile time using xmodmap.
///
/// It is not intended for use outside of that context and may be modified and updated without
/// announcing breaking API changes.
///
/// ```no_run
/// validate_user_bindings!(
///     ( "M-a", ... )
///     (
///         ( ( "M-{}", "M-S-{}" ) ( "1", "2", "3" ) )
///         ...
///     )
/// );
/// ```
#[proc_macro]
pub fn validate_user_bindings(input: TokenStream) -> TokenStream {
    let BindingsInput(mut bindings) = parse_macro_input!(input as BindingsInput);
    let names = keynames_from_xmodmap();
    let mut seen = HashSet::new();

    for b in bindings.iter_mut() {
        if seen.contains(&b.raw) {
            panic!("'{}' is bound as a keybinding more than once", b.raw);
        } else {
            seen.insert(&b.raw);
        }

        if b.keyname.is_none() {
            report_error("no key name specified", b)
        }

        if !is_valid_keyname(b, &names) {
            report_error(
                format!(
                    "'{}' is not a known key: run 'xmodmap -pke' to see valid key names",
                    b.keyname.take().unwrap()
                ),
                b,
            )
        }

        if !has_valid_modifiers(b) {
            report_error(
                format!(
                    "'{}' is an invalid modifer set: valid modifiers are {:?}",
                    b.mods.join("-"),
                    VALID_MODIFIERS
                ),
                b,
            );
        }
    }

    // If everything is fine then just consume the input
    TokenStream::new()
}
