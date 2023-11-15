use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam, Generics, Index,
};

#[proc_macro_derive(SizedOnDisk, attributes(dignore))]
pub fn derive_disk_size(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);

    // Used in the quasi-quotation below as `#name`.
    let name = input.ident;

    // Add a bound `T: SizedOnDisk` to every type parameter T.
    let generics = add_trait_bounds(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Generate an expression to sum up the heap size of each field.
    let sum = disk_size_sum(&input.data);

    let expanded = quote! {
        // The generated impl.
        impl #impl_generics crate::types::SizedOnDisk for #name #ty_generics #where_clause {
            fn size(&self) -> usize {
                #sum
            }
        }
    };

    // Hand the output tokens back to the compiler.
    proc_macro::TokenStream::from(expanded)
}

// Add a bound `T: SizedOnDisk` to every type parameter T.
fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(crate::types::SizedOnDisk));
        }
    }
    generics
}

// Generate an expression to sum up the heap size of each field.
fn disk_size_sum(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => {
            match data.fields {
                Fields::Named(ref fields) => {
                    // Expands to an expression like
                    //
                    //     0 + self.x.disk_size() + self.y.disk_size() + self.z.disk_size()
                    //
                    // but using fully qualified function call syntax.
                    //
                    // We take some care to use the span of each `syn::Field` as
                    // the span of the corresponding `size`
                    // call. This way if one of the field types does not
                    // implement `SizedOnDisk` then the compiler's error message
                    // underlines which field it is. An example is shown in the
                    // readme of the parent directory.
                    let attribute_name = "dignore";
                    let recurse = fields.named.iter()
                        .filter(|f| !f.attrs.iter().any(|a| 
                                                       a.meta.require_path_only()
                                                       .ok()
                                                       .filter(|p| p.is_ident(attribute_name)).is_some()
                                                       ))
                        .map(|f| {
                        let name = &f.ident;
                        quote_spanned! {f.span()=>
                            crate::types::SizedOnDisk::size(&self.#name)
                        }
                    });
                    quote! {
                        0 #(+ #recurse)*
                    }
                }
                Fields::Unnamed(ref fields) => {
                    // Expands to an expression like
                    //
                    //     0 + self.0.disk_size() + self.1.disk_size() + self.2.disk_size()
                    let recurse = fields.unnamed.iter().enumerate().map(|(i, f)| {
                        let index = Index::from(i);
                        quote_spanned! {f.span()=>
                            crate::types::SizedOnDisk::size(&self.#index)
                        }
                    });
                    quote! {
                        0 #(+ #recurse)*
                    }
                }
                Fields::Unit => {
                    // Unit structs cannot own more than 0 bytes of heap memory.
                    quote!(0)
                }
            }
        }
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}
