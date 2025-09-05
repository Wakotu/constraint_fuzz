// In your macro crate (e.g., my-derive)
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Field};

#[proc_macro_derive(EquivByLoc)]
pub fn derive_equiv_by_loc(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    // Get a reference to the fields of the struct.
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
