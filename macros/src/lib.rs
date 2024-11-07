use std::path::PathBuf;

use darling::{FromMeta, Result};
use proc_macro::TokenStream;
use quote::quote;
use syn::Fields;

#[proc_macro_derive(Wrapper)]
pub fn wrapper(input: TokenStream) -> TokenStream {
    let input_ = syn::parse_macro_input!(input as syn::DeriveInput);
    match process_input(input_) {
        Ok(x) => x,
        Err(err) => err.write_errors().into(),
    }
}

#[derive(Debug, Default, FromMeta)]
#[darling(allow_unknown_fields, default)]
struct SettingDotTomlOptions {
    setting: Option<String>,
    path: Option<PathBuf>,
}

#[proc_macro_derive(SettingDotToml, attributes(setting))]
pub fn api_setting(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match process_apisetting(input) {
        Ok(x) => x,
        Err(err) => err.write_errors().into(),
    }
}

fn process_apisetting(input: syn::DeriveInput) -> Result<TokenStream> {
    if let syn::Data::Struct(ref _struct) = &input.data {
        let name = &input.ident;

        let struct_attrs: Vec<_> = input
            .attrs
            .into_iter()
            .map(|attr| darling::ast::NestedMeta::Meta(attr.meta))
            .collect();
        let opt = <SettingDotTomlOptions as darling::FromMeta>::from_list(&struct_attrs)?;

        let path_i = opt.path.unwrap_or(PathBuf::from("setting.toml"));
        let path = path_i.to_string_lossy();

        let mut nested = quote! {};
        if let Some(setting) = opt.setting {
            let path_tokens: Vec<_> = setting.split('.').collect::<Vec<_>>();
            nested = quote! {
                let mut current = current;
                // Dynamically navigate through the TOML keys (local, question, etc.)
                for key in &[#(#path_tokens),*] {
                    if let toml::Value::Table(table) = current {
                        current = table.get(*key)
                            .expect("Key not found in TOML").clone();
                    }
                }
            }
        }

        return Ok(quote! {
            impl #name {
                async fn get() -> Self {
                    let path = std::path::PathBuf::from(#path);
                    let current = toml::from_str::<toml::Value>(
                        &tokio::fs::read_to_string(path)
                            .await
                            .expect("cant locate Setting.toml on project folder"),
                    )
                    .expect("the content of Setting.toml are invalid");

                    #nested

                    current.try_into().expect("failed to convert toml value to #name")
                }
            }
        }
        .into());
    }
    Err(darling::Error::custom(
        "`SettingDotToml` can only be derived for structs",
    ))
}

fn process_input(input: syn::DeriveInput) -> Result<TokenStream> {
    if let syn::Data::Struct(ref mstruct) = input.data {
        let name = &input.ident;
        // unnamed is field struct
        if let Fields::Unnamed(field) = &mstruct.fields {
            if let Some(inner) = field.unnamed.first() {
                let type_ = &inner.ty;
                return Ok(quote! {

                    impl std::ops::Deref for #name {
                        type Target = #type_;
                        fn deref(&self) -> &Self::Target {
                            &self.0
                        }
                    }

                    impl std::ops::DerefMut for #name {
                        fn deref_mut(&mut self) -> &mut Self::Target {
                            &mut self.0
                        }
                    }

                    impl From<#type_> for #name {
                        fn from(x:#type_) -> Self {
                            #name(x)
                        }
                    }
                }
                .into());
            }
        }
    }
    Err(darling::Error::custom(
        "`Wrapper` can only be derived for tuple structs (e.g., `struct MyStruct(T);`).",
    ))
}
