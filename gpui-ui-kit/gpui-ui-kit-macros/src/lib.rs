//! Proc macros for gpui-ui-kit
//!
//! Provides derive macros to reduce boilerplate in theme definitions.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Expr, Lit, Meta, Token};
use syn::punctuated::Punctuated;

/// Derive macro for component themes.
///
/// Generates `Default` and `From<&Theme>` implementations for theme structs.
///
/// # Attributes
///
/// Each field must have a `#[theme(...)]` attribute with:
/// - `default = 0xRRGGBB` or `default = 0xRRGGBBAA` - hex color for Default impl
/// - `from = field_name` - Theme field to map from (e.g., `accent`, `surface`)
///
/// For complex mappings, use `from_expr` instead:
/// - `from_expr = "expression"` - Custom expression using `theme` variable
///
/// For non-color fields (f32, etc.), use:
/// - `default_f32 = 0.5` - f32 literal for Default impl
/// - `from_expr = "0.5"` - constant value for From impl
///
/// # Example
///
/// ```ignore
/// use gpui_ui_kit_macros::ComponentTheme;
///
/// #[derive(ComponentTheme)]
/// pub struct ButtonTheme {
///     #[theme(default = 0x007acc, from = accent)]
///     pub accent: Rgba,
///
///     #[theme(default = 0x007acc33, from_expr = "with_alpha(theme.accent, 0.2)")]
///     pub accent_muted: Rgba,
///
///     #[theme(default_f32 = 0.5, from_expr = "0.5")]
///     pub disabled_opacity: f32,
/// }
/// ```
#[proc_macro_derive(ComponentTheme, attributes(theme))]
pub fn derive_component_theme(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("ComponentTheme only supports structs with named fields"),
        },
        _ => panic!("ComponentTheme only supports structs"),
    };

    let mut default_fields = Vec::new();
    let mut from_fields = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();

        // Find the #[theme(...)] attribute
        let theme_attr = field.attrs.iter().find(|attr| attr.path().is_ident("theme"));

        let Some(attr) = theme_attr else {
            panic!("Field `{}` is missing #[theme(...)] attribute", field_name);
        };

        let mut default_value: Option<u32> = None;
        let mut default_f32: Option<f64> = None;
        let mut from_field: Option<syn::Ident> = None;
        let mut from_expr: Option<String> = None;

        // Parse the attribute arguments
        let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .expect("Failed to parse theme attribute");

        for meta in nested {
            match meta {
                Meta::NameValue(nv) => {
                    let ident = nv.path.get_ident().expect("Expected identifier");
                    match ident.to_string().as_str() {
                        "default" => {
                            if let Expr::Lit(lit) = &nv.value {
                                if let Lit::Int(int_lit) = &lit.lit {
                                    default_value = Some(int_lit.base10_parse().unwrap());
                                }
                            }
                        }
                        "default_f32" => {
                            if let Expr::Lit(lit) = &nv.value {
                                match &lit.lit {
                                    Lit::Float(f) => {
                                        default_f32 = Some(f.base10_parse().unwrap());
                                    }
                                    Lit::Int(i) => {
                                        // Allow integers like 0 or 1
                                        default_f32 = Some(i.base10_parse::<i64>().unwrap() as f64);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        "from" => {
                            if let Expr::Path(path) = &nv.value {
                                from_field = path.path.get_ident().cloned();
                            }
                        }
                        "from_expr" => {
                            if let Expr::Lit(lit) = &nv.value {
                                if let Lit::Str(s) = &lit.lit {
                                    from_expr = Some(s.value());
                                }
                            }
                        }
                        _ => panic!("Unknown theme attribute: {}", ident),
                    }
                }
                _ => panic!("Expected name = value in theme attribute"),
            }
        }

        // Generate Default field based on type
        if let Some(f32_val) = default_f32 {
            // f32 field
            default_fields.push(quote! {
                #field_name: #f32_val as f32
            });
        } else if let Some(default_val) = default_value {
            // Check if it's RGB (6 hex digits) or RGBA (8 hex digits)
            let default_expr = if default_val > 0xFFFFFF {
                // RGBA - use rgba()
                quote! { gpui::rgba(#default_val) }
            } else {
                // RGB - use rgb()
                quote! { gpui::rgb(#default_val) }
            };

            default_fields.push(quote! {
                #field_name: #default_expr
            });
        } else {
            panic!(
                "Field `{}` is missing `default` or `default_f32` in #[theme(...)]",
                field_name
            );
        }

        // Generate From<&Theme> field
        if let Some(expr_str) = from_expr {
            let expr: syn::Expr = syn::parse_str(&expr_str)
                .expect(&format!("Failed to parse from_expr for field `{}`", field_name));
            from_fields.push(quote! {
                #field_name: #expr
            });
        } else if let Some(from) = from_field {
            from_fields.push(quote! {
                #field_name: theme.#from
            });
        } else {
            panic!(
                "Field `{}` needs either `from` or `from_expr` in #[theme(...)]",
                field_name
            );
        }
    }

    let expanded = quote! {
        impl Default for #name {
            fn default() -> Self {
                Self {
                    #(#default_fields),*
                }
            }
        }

        impl From<&crate::theme::Theme> for #name {
            fn from(theme: &crate::theme::Theme) -> Self {
                Self {
                    #(#from_fields),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}
