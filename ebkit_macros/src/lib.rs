extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DeriveInput, Fields, Ident, LitInt, LitStr, Type, parse_macro_input,
    spanned::Spanned,
};

#[proc_macro_derive(EvtDecode, attributes(evt, field))]
pub fn derive_evt_decode(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = input.ident.clone();

    let data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return syn::Error::new(input.span(), "EvtDecode can only be derived for enums")
                .to_compile_error()
                .into();
        }
    };

    let mut word_ty: Option<Type> = None;
    let mut tag_lsb: Option<u32> = None;
    let mut tag_width: Option<u32> = None;

    for attr in input.attrs.iter() {
        if attr.path().is_ident("evt") {
            let parse_result = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("word") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    let ty = lit.parse::<Type>()?;
                    word_ty = Some(ty);
                    return Ok(());
                }
                if meta.path.is_ident("tag_lsb") {
                    let value = meta.value()?;
                    let lit: LitInt = value.parse()?;
                    tag_lsb = Some(lit.base10_parse::<u32>()?);
                    return Ok(());
                }
                if meta.path.is_ident("tag_width") {
                    let value = meta.value()?;
                    let lit: LitInt = value.parse()?;
                    tag_width = Some(lit.base10_parse::<u32>()?);
                    return Ok(());
                }
                Err(meta.error("Unsupported evt attribute"))
            });
            if let Err(err) = parse_result {
                return err.to_compile_error().into();
            }
        }
    }

    let word_ty = match word_ty {
        Some(ty) => ty,
        None => {
            return syn::Error::new(input.span(), "Missing #[evt(word = \"u32\")] attribute")
                .to_compile_error()
                .into();
        }
    };
    let word_bits = match word_ty_bits(&word_ty) {
        Ok(bits) => bits,
        Err(err) => return err.to_compile_error().into(),
    };
    let tag_lsb = match tag_lsb {
        Some(val) => val,
        None => {
            return syn::Error::new(input.span(), "Missing #[evt(tag_lsb = N)] attribute")
                .to_compile_error()
                .into();
        }
    };
    let tag_width = match tag_width {
        Some(val) => val,
        None => {
            return syn::Error::new(input.span(), "Missing #[evt(tag_width = N)] attribute")
                .to_compile_error()
                .into();
        }
    };
    if tag_width == 0
        || tag_lsb >= word_bits
        || tag_width > word_bits
        || tag_lsb + tag_width > word_bits
    {
        return syn::Error::new(input.span(), "tag field out of range for word size")
            .to_compile_error()
            .into();
    }

    let mut match_arms = Vec::new();
    let mut unknown_error = None;

    for variant in data.variants.iter() {
        let attrs = &variant.attrs;
        let variant_tags = find_evt_tags(attrs);

        if variant_tags.is_empty() {
            unknown_error = Some(syn::Error::new(
                variant.span(),
                "Each variant must have #[evt(tag = N)]",
            ));
            break;
        }

        let field_build = match build_variant_constructor(variant, word_bits) {
            Ok(tokens) => tokens,
            Err(err) => {
                return err.to_compile_error().into();
            }
        };

        for variant_tag in variant_tags {
            match_arms.push(quote! { #variant_tag => { #field_build } });
        }
    }

    if let Some(err) = unknown_error {
        return err.to_compile_error().into();
    }

    let decode_fn = format_ident!("decode");

    let expanded = quote! {
        impl #enum_name {
            pub fn #decode_fn(word: #word_ty) -> Option<Self> {
                let tag = {
                    let mask = if #tag_width == 64 {
                        u64::MAX
                    } else {
                        (1u64 << #tag_width) - 1
                    };
                    ((u64::from(word) >> #tag_lsb) & mask) as u32
                };
                match tag {
                    #(#match_arms,)*
                    _ => None,
                }
            }
        }
    };

    expanded.into()
}

fn find_evt_tags(attrs: &[Attribute]) -> Vec<u32> {
    let mut tags = Vec::new();
    for attr in attrs.iter() {
        if attr.path().is_ident("evt") {
            let parse_result = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("tag") {
                    let value = meta.value()?;
                    let lit: LitInt = value.parse()?;
                    tags.push(lit.base10_parse::<u32>()?);
                    return Ok(());
                }
                Ok(())
            });
            if parse_result.is_err() {
                return Vec::new();
            }
        }
    }
    tags
}

fn build_variant_constructor(
    variant: &syn::Variant,
    word_bits: u32,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    let fields = match &variant.fields {
        Fields::Named(fields) => &fields.named,
        Fields::Unit => {
            let variant_ident = &variant.ident;
            return Ok(quote! { Ok(Self::#variant_ident) });
        }
        Fields::Unnamed(_) => {
            return Err(syn::Error::new(
                variant.span(),
                "Tuple variants are not supported; use named fields",
            ));
        }
    };

    let mut field_inits = Vec::new();
    for field in fields.iter() {
        let ident = field.ident.as_ref().expect("named field");
        let (lsb, width) = parse_field_attr(&field.attrs, ident, word_bits)?;
        let ty = &field.ty;
        field_inits.push(quote! {
            #ident: {
                let mask = if #width == 64 { u64::MAX } else { (1u64 << #width) - 1 };
                ((u64::from(word) >> #lsb) & mask) as #ty
            }
        });
    }

    let variant_ident = &variant.ident;
    Ok(quote! {
        Some(Self::#variant_ident { #(#field_inits,)* })
    })
}

fn parse_field_attr(
    attrs: &[Attribute],
    ident: &Ident,
    word_bits: u32,
) -> Result<(u32, u32), syn::Error> {
    for attr in attrs.iter() {
        if attr.path().is_ident("field") {
            let mut lsb = None;
            let mut width = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("lsb") {
                    let value = meta.value()?;
                    let lit: LitInt = value.parse()?;
                    lsb = Some(lit.base10_parse::<u32>()?);
                    return Ok(());
                }
                if meta.path.is_ident("width") {
                    let value = meta.value()?;
                    let lit: LitInt = value.parse()?;
                    width = Some(lit.base10_parse::<u32>()?);
                    return Ok(());
                }
                Err(meta.error("invalid field syntax"))
            })?;
            let lsb = lsb.ok_or_else(|| syn::Error::new(attr.span(), "field missing lsb"))?;
            let width = width.ok_or_else(|| syn::Error::new(attr.span(), "field missing width"))?;
            if width == 0 || lsb >= word_bits || width > word_bits || lsb + width > word_bits {
                return Err(syn::Error::new(
                    attr.span(),
                    "field out of range for word size",
                ));
            }
            return Ok((lsb, width));
        }
    }
    Err(syn::Error::new(
        ident.span(),
        "Each field must have #[field(lsb = N, width = N)]",
    ))
}

fn word_ty_bits(word_ty: &Type) -> Result<u32, syn::Error> {
    if let Type::Path(type_path) = word_ty {
        if type_path.qself.is_none() && type_path.path.segments.len() == 1 {
            let ident = &type_path.path.segments[0].ident;
            if ident == "u16" {
                return Ok(16);
            }
            if ident == "u32" {
                return Ok(32);
            }
            if ident == "u64" {
                return Ok(64);
            }
        }
    }
    Err(syn::Error::new(
        word_ty.span(),
        "word type must be u16, u32, or u64",
    ))
}
