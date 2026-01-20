use inflector;
use proc_macro::TokenStream;
use quote::{format_ident, quote}; // , quote_spanned};
use syn;

#[proc_macro_derive(InPlaceInit)]
pub fn derive_in_place_set(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    let tokens = generate_in_place_set(input);
    tokens.into()
}
fn generate_in_place_set(input: syn::DeriveInput) -> quote::__private::TokenStream {
    let syn::DeriveInput {
        attrs: _,
        vis,
        ident,
        generics,
        data: _,
    } = input;
    let local_data = if let syn::Data::Enum(data) = input.data {
        data
    } else {
        panic!("Can't do non-enum in-place setter");
        // ::std::compile_error!("Can't derive in-pace setter for non-enum");
    };

    let mod_name = snake_ident(&format_ident!("{}_internal", ident));
    let tag_name = format_ident!("{}Tag", ident);
    let union_name = format_ident!("{}Union", ident);

    let variants: Vec<syn::Variant> = local_data.variants.into_iter().collect();

    let impl_generics = generics.clone();
    let impl_generics2 = generics.clone();

    let tag_variants: Vec<syn::Variant> = variants
        .iter()
        .map(|var| syn::Variant {
            attrs: var.attrs.clone(),
            ident: var.ident.clone(),
            fields: syn::Fields::Unit,
            discriminant: var.discriminant.clone(),
        })
        .collect();

    let repr_c: syn::Attribute = syn::Attribute {
        pound_token: syn::token::Pound {
            spans: [proc_macro2::Span::call_site()],
        }, // syn::Token![#],
        style: syn::AttrStyle::Outer,
        bracket_token: syn::token::Bracket(proc_macro2::Span::call_site()),
        path: syn::parse_quote! { repr },
        tokens: syn::parse_quote! { (C) },
    };
    let structs = variants.iter().map(|var| syn::ItemStruct {
        attrs: [repr_c.clone()].into(),
        vis: vis.clone(),
        struct_token: syn::parse2(quote! { struct }).unwrap(),
        ident: var.ident.clone(),
        generics: filter_generics_for_use(&var.fields, &generics),
        fields: prepend_tag_field(&tag_name, &var.fields),
        semi_token: None,
    });
    let filtered_generics_full: Vec<syn::Generics> = variants
        .iter()
        .map(|var| filter_generics_for_use(&var.fields, &generics))
        .collect();
    let filtered_generics = filtered_generics_full
        .iter()
        .map(|gen| gen.split_for_impl().1);
    let struct_names = variants.iter().map(|var| var.ident.clone());
    let union_field_names = variants.iter().map(|var| snake_ident(&var.ident));

    let funcs = variants.iter().map(|variant| {
        let var_name = &variant.ident.clone();
        let name = snake_ident(&variant.ident);
        let init_name = format_ident!("init_{}", name);
        let type_name = ident.clone();
        let local_tag_name = tag_name.clone();
        let generics = generics_just_vars(&impl_generics);
        match variant.fields {
            syn::Fields::Named(syn::FieldsNamed { ref named, brace_token: _ }) => {
                let _named_iter = named.iter();
                syn::parse_quote! { }
            }
            syn::Fields::Unnamed(syn::FieldsUnnamed { ref unnamed, paren_token: _ }) => {
                let vns = unnamed.iter().enumerate().map(|(i, _)| {
                    format_ident!("i{}", i)
                });
                let field_nos = unnamed.iter().enumerate().map(|(i, _)| {
                    syn::Index::from(i+1)
                });
                let vp = unnamed.iter().enumerate().map(|(i, syn::Field { ty: f, .. })| {
                    let vn = format_ident!("i{}", i);
                    quote! {
                        #vn : impl FnOnce(*mut core::mem::MaybeUninit<#f>)
                    }});
                let fs = unnamed.iter().map(|syn::Field { ty: f, .. }| {
                    f
                });
                syn::parse_quote! {
                    #[inline(never)]
                fn #init_name(a: *mut core::mem::MaybeUninit<#type_name #generics>, #(#vp),*) {
                    unsafe {
                        let b: *mut core::mem::MaybeUninit<#mod_name::#union_name #generics> = core::mem::transmute(a);
                        core::ptr::addr_of_mut!((*(*(*b).as_mut_ptr()).#name).0).write(#mod_name::#local_tag_name::#var_name);
                        // MUST NOT panic or fail to fill it's argument.
                        #(#vns(core::ptr::addr_of_mut!((*(*(*b).as_mut_ptr()).#name).#field_nos) as *mut core::mem::MaybeUninit<#fs>);)*
                    }
                }
                }
            }
            syn::Fields::Unit => {
                syn::parse_quote! {
                    fn #init_name(a: *mut core::mem::MaybeUninit<#type_name #generics>) {
                        unsafe {
                            let b : *mut core::mem::MaybeUninit<#mod_name::#union_name #generics> = core::mem::transmute(a);
                            core::ptr::addr_of_mut!((*(*(*b).as_mut_ptr()).#name).__tag).write(#mod_name::#local_tag_name::#var_name);
                        }
                    }
                }
            }
        }
    });

    let impl_type_generics_top = generics.split_for_impl().1;

    let the_impl = syn::ItemImpl {
        attrs: Vec::new(),
        defaultness: None,
        unsafety: None,
        impl_token: syn::Token![impl](proc_macro2::Span::call_site()),
        generics: impl_generics2,
        trait_: None,
        self_ty: syn::parse_quote! { #ident #impl_type_generics_top },
        brace_token: syn::token::Brace(proc_macro2::Span::call_site()),
        items: funcs.collect(),
    };

    let generic_where = impl_generics.where_clause.clone();

    quote! {
            #vis mod #mod_name {
                use super::{*};
    //            #[derive(Debug)]
                #[repr(u8)]
                pub enum #tag_name {
                    #(#tag_variants),*
                }
                #(#structs)*
                #[repr(C)]
                pub union #union_name #impl_generics #generic_where {
                    #(pub #union_field_names: core::mem::ManuallyDrop<#struct_names #filtered_generics>),*
                }
            }
            #the_impl
            /*impl #impl_generics #ident #impl_generics2 {
               #(#funcs)*
            }*/
        }
}

/*fn camel_ident(ident: &proc_macro2::Ident) -> proc_macro2::Ident {
    proc_macro2::Ident::new(&inflector::cases::camelcase::to_camel_case(ident.to_string()), ident.span())
} */

fn snake_ident(ident: &proc_macro2::Ident) -> proc_macro2::Ident {
    proc_macro2::Ident::new(
        &inflector::cases::snakecase::to_snake_case(ident.to_string()),
        ident.span(),
    )
}

fn generics_just_vars(generics: &syn::Generics) -> syn::TypeGenerics {
    generics.split_for_impl().1
}

fn prepend_tag_field(tag: &syn::Ident, fields: &syn::Fields) -> syn::Fields {
    match fields {
        syn::Fields::Named(syn::FieldsNamed {
            named,
            brace_token: _,
        }) => {
            let named_iter = named.iter();
            let new_fields: syn::FieldsNamed =
                syn::parse_quote! { { pub __tag: #tag, #(pub #named_iter),* } };
            new_fields.into()
        }
        syn::Fields::Unnamed(syn::FieldsUnnamed {
            unnamed,
            paren_token: _,
        }) => {
            let unnamed_iter = unnamed.iter();
            let new_fields: syn::FieldsUnnamed =
                syn::parse_quote! { (pub #tag, #(pub #unnamed_iter),*) };
            new_fields.into()
        }
        syn::Fields::Unit => {
            let new_fields: syn::FieldsNamed = syn::parse_quote! { { pub __tag: #tag } };
            new_fields.into()
        }
    }
}

fn filter_generics_for_use(fields: &syn::Fields, generics: &syn::Generics) -> syn::Generics {
    // generics.iter.clone() // FIXME WRONG: actually filter for use.
    let filtered_params = generics
        .params
        .iter()
        .filter(|generic| {
            fields
                .iter()
                .any(|field| search_type_for_generic(&field.ty, generic))
        })
        .map(|x| x.clone())
        .collect();
    syn::Generics {
        lt_token: generics.lt_token.clone(),
        params: filtered_params,
        gt_token: generics.gt_token.clone(),
        where_clause: generics.where_clause.clone(),
    }
}

fn search_type_for_generic(ty: &syn::Type, generic_in: &syn::GenericParam) -> bool {
    match generic_in {
        syn::GenericParam::Type(generic) => {
            match ty {
                syn::Type::Path(ty) => {
                    // Not enough yet; need to check parameters too.
                    ty.path.segments.iter().any(|x| {
                        x.ident == generic.ident
                            || search_path_arguments_for_generic(&x.arguments, generic_in)
                    }) || match &ty.qself {
                        None => false,
                        Some(qs) => search_type_for_generic(qs.ty.as_ref(), generic_in),
                    }
                }
                _ => true,
            }
        }
        _ => true,
    }
}

fn search_path_arguments_for_generic(
    path: &syn::PathArguments,
    generic_in: &syn::GenericParam,
) -> bool {
    use syn::PathArguments::*;
    match generic_in {
        syn::GenericParam::Type(_generic) => {
            match path {
                None => false,
                AngleBracketed(path) => path.args.iter().any(|ga| {
                    use syn::GenericArgument::*;
                    match ga {
                        Lifetime(_) => false,
                        Type(ty) => search_type_for_generic(ty, generic_in),
                        Binding(_) => false,
                        Constraint(_) => false,
                        Const(_expr) => false,
                    }
                }),
                Parenthesized(path) => {
                    use syn::ReturnType::*;
                    path.inputs
                        .iter()
                        .any(|ty| search_type_for_generic(ty, generic_in))
                        || match &path.output {
                            Default => false, // Can't really infer to a generic argument in a type declaration, I think.
                            Type(_, rv_ty) => search_type_for_generic(&*rv_ty, generic_in),
                        }
                }
            }
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
