//! Implementation of a macro creating the template specification.
//!
//! Some code is taken from [pest](https://github.com/pest-parser/pest/).
#![recursion_limit = "256"]

extern crate proc_macro;
extern crate proc_macro2;
extern crate syn;
#[macro_use]
extern crate quote;
extern crate serde;
extern crate serde_yaml;
#[macro_use]
extern crate serde_derive;

use std::env;
use std::path::{Path};
use proc_macro2::{Span, TokenStream};
use syn::{DeriveInput, Ident, LitStr};

mod spec;
mod util;

use util::*;
use spec::{SpecTemplate, SpecPriority, SpecFormat};

fn check_template(template: &SpecTemplate) -> (Ident, Vec<LitStr>, Ident, LitStr) {
    let first_uppercase = template.identifier.chars().next()
        .map(|c| c.is_uppercase()).unwrap_or(false);

    if !first_uppercase {
        panic!("first character of identifier {:?} \
            should be uppercase!", template.identifier);
    }

    if template.names.is_empty() {
        panic!("{:?}: templates must have at least one name!",
               template.identifier);
    }

    for child in &template.attributes {
        if child.identifier.chars().any(|c| c.is_uppercase()) {
            panic!("{:?}: attribute identifiers should be lowercase!",
                child.identifier)
        }

        if child.names.is_empty() {
            panic!("{:?}: attributes must have at least one name!",
                   child.identifier);
        }
    }

    let name: Ident = Ident::new(&template.identifier, Span::call_site());
    let names = str_to_lower_lit(&template.names);
    let format = match template.format {
        SpecFormat::Inline => Ident::new("Inline", Span::call_site()),
        SpecFormat::Block => Ident::new("Block", Span::call_site()),
        SpecFormat::Box => Ident::new("Box", Span::call_site()),
    };
    let description = LitStr::new(&template.description, Span::call_site());
    (name, names, format, description)
}

fn implement_template_id(templates: &[SpecTemplate]) -> TokenStream {
    let variants: Vec<Ident> = templates.iter().map(|template| {
        let (name, _, _, _) = check_template(template);
        name
    }).collect();
    let enum_variants = variants.iter().map(|name|
        quote! {
            #name(#name<'e>)
        }
    );
    let present_variants = variants.iter();
    quote! {
        /// The available template types.
        #[derive(Debug, Clone, PartialEq, Serialize)]
        pub enum KnownTemplate<'e> {
            #( #enum_variants ),*
        }

        impl<'e> KnownTemplate<'e> {
            pub fn present(&self) -> &Vec<Attribute<'e>> {
                match *self {
                    #( KnownTemplate::#present_variants(ref t) => &t.present ),*
                }
            }
            pub fn find(&self, name: &str) -> Option<&Attribute<'e>> {
                for attribute in self.present() {
                    if &attribute.name == name {
                        return Some(attribute)
                    }
                }
                None
            }
        }
    }
}

fn str_to_lower_lit(input: &[String]) -> Vec<LitStr> {
    input
        .iter()
        .map(|a| LitStr::new(&a.trim().to_lowercase(), Span::call_site()))
        .collect()
}

fn priority_to_ident(prio: SpecPriority) -> Ident {
    match prio {
        SpecPriority::Required => Ident::new("Required", Span::call_site()),
        SpecPriority::Optional => Ident::new("Optional", Span::call_site()),
    }
}

fn implement_attribute_spec(template: &SpecTemplate) -> Vec<TokenStream> {
    template.attributes.iter().map(|attribute| {
        let names = str_to_lower_lit(&attribute.names);
        let priority = priority_to_ident(attribute.priority);
        let predicate = Ident::new(&attribute.predicate, Span::call_site());
        let description = LitStr::new(&attribute.description, Span::call_site());
        let pred_name = LitStr::new(&attribute.predicate, Span::call_site());
        quote! {
            AttributeSpec {
                names: vec![ #( #names.into() ),*],
                priority: Priority::#priority,
                predicate: &#predicate,
                predicate_name: #pred_name.into(),
                description: #description.into(),
            }
        }
    }).collect()
}

fn implement_spec_list(templates: &[SpecTemplate]) -> TokenStream {
    let specs = templates.iter().map(|template| {
        let (_, names, format, description) = check_template(template);
        let attributes = implement_attribute_spec(template);
        quote! {
            TemplateSpec {
                names: vec![ #( #names.into() ),* ],
                description: #description.into(),
                format: Format::#format,
                attributes: vec![ #( #attributes ),* ]
            }
        }
    });
    quote! {
        /// A representation of all templates in )the specification.
        pub fn spec<'p>() -> Vec<TemplateSpec<'p>> {
            vec![ #( #specs ),* ]
        }
    }
}

fn implement_parsing_match(template: &SpecTemplate) -> TokenStream {
    let (name, names, format, description) = check_template(template);
    let attributes = template.attributes.iter().map(|attr| {
        let attr_name = Ident::new(&attr.identifier, Span::call_site());
        let alt_names = str_to_lower_lit(&attr.names);
        match attr.priority {
            SpecPriority::Required => quote! {
                #attr_name: {
                    // abort template parsing if required argument is missing.
                    if let Some(c) = extract_content(&[ #( #alt_names.into() ),* ]) {
                        c
                    } else {
                        return None
                    }
                }
            },
            SpecPriority::Optional => quote! {
                #attr_name: extract_content(&[ #( #alt_names.into() ),* ])
            }
        }
    });
    let present = template.attributes.iter().map(|attr| {
        let alt_names = str_to_lower_lit(&attr.names);
        let att_name = LitStr::new(&attr.identifier, Span::call_site());
        let priority = priority_to_ident(attr.priority);
        quote! {
            if let Some(value) = extract_content(&[ #( #alt_names.into() ),* ]) {
                present.push(Attribute {
                    name: #att_name.into(),
                    priority: Priority::#priority,
                    value,
                });
            }
        }
    });
    quote! {
        let names = vec![#( #names.trim().to_lowercase() ),*];
        if names.contains(&name) {
            let template = #name {
                names: names,
                description: #description.into(),
                format: Format::#format,
                #( #attributes ),*,
                present: {
                    let mut present = vec![];
                    #( #present )*
                    present
                }
            };
            return Some(KnownTemplate::#name(template));
        }
    }
}

fn implement_template_parsing(templates: &[SpecTemplate]) -> TokenStream {

    let template_kinds = templates.iter()
        .map(|t| implement_parsing_match(t));

    quote! {
        /// Try to create a `KnownTemplate` variant from an element, using the specification.
        pub fn parse_template<'e>(template: &'e Template) -> Option<KnownTemplate<'e>> {
            let extract_content = | attr_names: &[String] | {
                if let Some(arg) = find_arg(&template.content, attr_names) {
                    if let Element::TemplateArgument(ref arg) = *arg {
                        return Some(arg.value.as_slice())
                    }
                }
                None
            };

            let name = extract_plain_text(&template.name).trim().to_lowercase();
            #( #template_kinds )*
            None
        }
    }
}

fn implement_templates(templates: &[SpecTemplate]) -> Vec<TokenStream> {
    templates.iter().map(|template| {

        let (name, names, _, _) = check_template(template);
        let description = template.description.split('\n')
            .map(|l| LitStr::new(&l, Span::call_site()));
        let attribute_impls = template.attributes.iter().map(|attr| {
            let attr_id: Ident = Ident::new(&attr.identifier, Span::call_site());
            let description = attr.description.split('\n')
                .map(|l| LitStr::new(&l, Span::call_site()));
            match attr.priority {
                SpecPriority::Required => quote! {
                    #( #[doc = #description] )*
                    pub #attr_id: &'e [Element]
                },
                SpecPriority::Optional => quote! {
                    #( #[doc = #description] )*
                    pub #attr_id: Option<&'e [Element]>
                },
            }
        });

        quote! {
            #[derive(Debug, Clone, PartialEq, Serialize)]
            #( #[doc = #description] )*
            ///
            /// Alternative names:
            #( #[doc = #names ] )*
            pub struct #name<'e> {
                pub names: Vec<String>,
                pub format: Format,
                pub description: String,
                pub present: Vec<Attribute<'e>>,
                # (#attribute_impls ),*
            }
        }
    }).collect()
}

#[proc_macro_derive(TemplateSpec, attributes(spec))]
pub fn create_template_spec(input: proc_macro::TokenStream) -> proc_macro::TokenStream {

    let ast: DeriveInput = syn::parse(input.into()).unwrap();
    let (_name, path) = parse_derive(&ast);

    let root = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let path = Path::new(&root).join("src/").join(&path);
    let file_name = match path.file_name() {
        Some(file_name) => file_name,
        None => panic!("spec attribute should point to a file")
    };

    let data = match read_file(&path) {
        Ok(data) => data,
        Err(error) => panic!("error opening {:?}: {}", file_name, error)
    };
    let templates: Vec<SpecTemplate> = serde_yaml::from_str(&data)
        .expect("cannot parse spec:");

    let template_id = implement_template_id(&templates);
    let template_impls = implement_templates(&templates);
    let spec_func = implement_spec_list(&templates);
    let template_parsing = implement_template_parsing(&templates);

    let implementation = quote! {
        /// Types and utils used in the documentation.
        pub mod spec_meta {
            /// Specifies wether a template represents a logical unit (`Block`)
            /// or simpler markup (`Inline`).
            #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
            pub enum Format {
                Block,
                Box,
                Inline
            }

            /// Template attributes can have different priorities.
            #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
            pub enum Priority {
                Required,
                Optional
            }

            /// Represents failure of a predicate check.
            pub struct PredError<'e> {
                pub tree: Option<&'e Element>,
                pub cause: String,
            }

            /// Result of a predicate check.
            pub type PredResult<'e> = Result<(), PredError<'e>>;
            /// A function to determine wether a given element is allowed.
            pub type Predicate = Fn(&[Element]) -> PredResult + Sync;

            /// Checks a predicate for a given input tree.
            struct TreeChecker<'path, 'e> {
                pub path: Vec<&'path Element>,
                pub result: PredResult<'e>,
            }

            impl <'e, 'p: 'e> Traversion<'e, &'p Predicate> for TreeChecker<'e, 'e> {

                path_methods!('e);

                fn work_vec(
                    &mut self,
                    root: &'e [Element],
                    predicate: &'p Predicate,
                    _: &mut io::Write
                ) -> io::Result<bool> {
                    if self.result.is_err() {
                        return Ok(false)
                    }
                    self.result = (predicate)(root);
                    Ok(true)
                }
            }

            /// Checks a predicate recursively.
            pub fn always<'e, 'p: 'e>(root: &'e [Element], predicate: &'p Predicate)
                -> PredResult<'e>
            {
                let mut checker = TreeChecker {
                    path: vec![],
                    result: Ok(()),
                };
                checker.result = Ok(());
                checker.run_vec(&root, predicate, &mut vec![])
                    .expect("error checking predicate!");
                checker.result
            }


            /// Represents a (semantic) template.
            #[derive(Clone, Serialize)]
            pub struct TemplateSpec<'p> {
                pub names: Vec<String>,
                pub description: String,
                pub format: Format,
                pub attributes: Vec<AttributeSpec<'p>>,
            }

            /// Represents the specification of an attribute (or argument) of a template.
            #[derive(Clone, Serialize)]
            pub struct AttributeSpec<'p> {
                pub names: Vec<String>,
                pub description: String,
                pub priority: Priority,
                #[serde(skip)]
                pub predicate: &'p Predicate,
                pub predicate_name: String,
            }

            impl<'p> TemplateSpec<'p> {
                /// Returns the default / preferred name of this template.
                /// This is the first name in the list.
                pub fn default_name(&self) -> &str {
                    self.names.first().unwrap()
                }
            }

            impl<'p> AttributeSpec<'p> {
                /// Returns the default / preferred name of this attribute.
                /// This is the first name in the list.
                pub fn default_name(&self) -> &str {
                    self.names.first().unwrap()
                }
            }

            /// Represents a concrete value of a template attribute.
            #[derive(Debug, Clone, PartialEq, Serialize)]
            pub struct Attribute<'e> {
                pub name: String,
                pub priority: Priority,
                pub value: &'e [Element],
            }
        }

        use spec_meta::*;

        /// Get the specification of a specific template, if it exists.
        pub fn spec_of<'p>(name: &str) -> Option<TemplateSpec<'p>> {
            let name = name.trim().to_lowercase();
            for spec in spec() {
                if spec.names.contains(&name) {
                    return Some(spec)
                }
            }
            None
        }

        #template_id
        #spec_func
        #template_parsing
        #( #template_impls )*
    };
    implementation.into()
}


