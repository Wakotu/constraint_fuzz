// In your macro crate (e.g., my-derive)
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Field};

#[proc_macro_derive(EquivByLoc)]
pub fn derive_equiv_by_loc(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    // Get name of the struct
    let name = &input.ident;
    // Check derived type is a struct and Get iterator of its fields
    let fields = match &input.data {
        Data::Struct(data_struct) => &data_struct.fields,
        _ => panic!("EquivByLoc can only be derived for structs with named fields"),
    };

    // Find the `loc` field and check its type.
    let _: &Field = fields
        .iter()
        .find(|field| field.ident.as_ref().unwrap() == "loc")
        .unwrap_or_else(|| {
            panic!("EquivByLoc requires a field named `loc`");
        });
    // The generated code for the `PartialEq` implementation
    let eq_impl = quote! {
        impl PartialEq for #name {
            fn eq(&self, other: &Self) -> bool {
                self.loc == other.loc
            }
        }
    };

    // The generated code for the `Eq` implementation
    let eq_marker_impl = quote! {
        impl Eq for #name {}
    };

    let hash_impl = quote! {
        impl std::hash::Hash for #name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.loc.hash(state);
            }
        }
    };

    // Combine the generated code and return it
    let expanded = quote! {
        #eq_impl
        #eq_marker_impl
        #hash_impl
    };

    TokenStream::from(expanded)
    // expanded
}

#[proc_macro_derive(EquivByName)]
pub fn derive_equiv_by_name(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    // Get name of the struct
    let name = &input.ident;
    // Check derived type is a struct and Get iterator of its fields
    let fields = match &input.data {
        Data::Struct(data_struct) => &data_struct.fields,
        _ => panic!("EquivByName can only be derived for structs with named fields"),
    };

    // Find the `name` field and check its type.
    let _: &Field = fields
        .iter()
        .find(|field| {
            let flag = field.ident.as_ref().unwrap() == "name";
            if !flag {
                return false;
            }
            if let syn::Type::Path(type_path) = &field.ty {
                if type_path.qself.is_none()
                    && type_path.path.segments.len() == 1
                    && type_path.path.segments[0].ident == "String"
                    && type_path.path.segments[0].arguments.is_empty()
                {
                    return true;
                }
            }
            false
        })
        .unwrap_or_else(|| {
            panic!("EquivByName requires a field named `name`");
        });
    // The generated code for the `PartialEq` implementation
    let eq_impl = quote! {
        impl PartialEq for #name {
            fn eq(&self, other: &Self) -> bool {
                self.name == other.name
            }
        }
    };

    // The generated code for the `Eq` implementation
    let eq_marker_impl = quote! {
        impl Eq for #name {}
    };

    let hash_impl = quote! {
        impl std::hash::Hash for #name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.name.hash(state);
            }
        }
    };

    let borrow_impl = quote! {
        impl std::borrow::Borrow<str> for #name {
            fn borrow(&self) -> &str {
                &self.name
            }
        }
    };

    // Combine the generated code and return it
    let expanded = quote! {
        #eq_impl
        #eq_marker_impl
        #hash_impl
        #borrow_impl
    };

    TokenStream::from(expanded)
    // expanded
}
