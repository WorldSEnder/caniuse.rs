use std::{
    collections::{BTreeMap, BTreeSet},
    default::Default,
    env,
    error::Error,
    fmt::Debug,
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
};

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FeatureToml {
    versions: Vec<FeatureList>,
    unstable: FeatureList,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct VersionData {
    /// Rust version number, e.g. "1.0.0"
    number: String,
    /// The channel (stable / beta / nightly)
    #[serde(default)]
    channel: Channel,
    /// Blog post path (https://blog.rust-lang.org/{path})
    blog_post_path: Option<String>,
    /// GitHub milestone id (https://github.com/rust-lang/rust/milestone/{id})
    gh_milestone_id: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FeatureList {
    #[serde(flatten)]
    version: Option<VersionData>,
    /// List of features (to be) stabilized in this release
    #[serde(default)]
    features: Vec<FeatureData>,
}

/// A "feature", as tracked by this app. Can be a nightly Rust feature, a
/// stabilized API, or anything else that one version of Rust (deliberately)
/// supports while a previous one didn't support it.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct FeatureData {
    /// Short description to identify the feature
    title: String,
    /// Feature flag name, for things that were previously or are still Rust
    /// nightly features with such a thing (`#![feature(...)]`)
    flag: Option<String>,
    /// Feature slug, used for the permalink. If a feature flag exists, this
    /// can be omitted, then the flag is used for the permalink.
    slug: Option<String>,
    /// RFC id (https://github.com/rust-lang/rfcs/pull/{id})
    rfc_id: Option<u64>,
    /// Implementation PR id (https://github.com/rust-lang/rust/pull/{id})
    ///
    /// Only for small features that were implemented in one PR.
    impl_pr_id: Option<u64>,
    /// Tracking issue id (https://github.com/rust-lang/rust/issues/{id})
    tracking_issue_id: Option<u64>,
    /// Stabilization PR id (https://github.com/rust-lang/rust/pull/{id})
    stabilization_pr_id: Option<u64>,
    /// Documentation path (https://doc.rust-lang.org/{path})
    doc_path: Option<String>,
    /// Edition guide path (https://doc.rust-lang.org/edition-guide/{path})
    edition_guide_path: Option<String>,
    /// Unstable book path (https://doc.rust-lang.org/unstable-book/{path})
    unstable_book_path: Option<String>,
    /// Language items (functions, structs, modules) that are part of this
    /// feature (unless this feature is exactly one item and that item is
    /// already used as the title)
    #[serde(default)]
    items: Vec<String>,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Channel {
    Stable,
    Beta,
    Nightly,
}

/// Not specifying the channel in features.toml is equivalent to specifying
/// "stable"
impl Default for Channel {
    fn default() -> Self {
        Self::Stable
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=features.toml");
    println!("cargo:rerun-if-changed=templates/index.html");
    println!("cargo:rerun-if-changed=templates/nightly.html");
    println!("cargo:rerun-if-changed=templates/skel.html");

    let features_raw = fs::read("features.toml")?;
    let feature_toml: FeatureToml = toml::from_slice(&features_raw)?;

    // TODO: Add a filter that replaces `` by <code></code>
    let tera = Tera::new("templates/*")?;
    let ctx = Context::from_serialize(&feature_toml)?;
    fs::write("public/index.html", tera.render("index.html", &ctx)?)?;
    fs::write("public/nightly.html", tera.render("nightly.html", &ctx)?)?;

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("features.rs");
    let mut out = BufWriter::new(File::create(out_path)?);

    write!(out, "{}", generate_data(feature_toml))?;

    Ok(())
}

fn generate_data(feature_toml: FeatureToml) -> TokenStream {
    let mut monogram_index = BTreeMap::new();
    let mut bigram_index = BTreeMap::new();
    let mut trigram_index = BTreeMap::new();

    let mut versions = Vec::new();

    let mut feat_idx = 0;

    for v in feature_toml.versions {
        if let Some(d) = &v.version {
            let number = &d.number;
            let channel = Ident::new(&format!("{:?}", d.channel), Span::call_site());
            let blog_post_path = option_literal(&d.blog_post_path);
            let gh_milestone_id = option_literal(&d.gh_milestone_id);

            versions.push(quote! {
                VersionData {
                    number: #number,
                    channel: Channel::#channel,
                    blog_post_path: #blog_post_path,
                    gh_milestone_id: #gh_milestone_id,
                }
            });
        }

        for f in v.features {
            assert!(
                !f.items.iter().any(|i| i.contains('`')),
                "items are always wrapped in code blocks and should not contain any '`'.",
            );

            add_feature_ngrams(1, &mut monogram_index, &f, feat_idx);
            add_feature_ngrams(2, &mut bigram_index, &f, feat_idx);
            add_feature_ngrams(3, &mut trigram_index, &f, feat_idx);

            feat_idx += 1;
        }
    }

    let versions = quote! {
        pub const VERSIONS: &[VersionData] = &[#(#versions),*];
    };

    let monogram_index_insert_stmts = monogram_index.into_iter().map(|(k, v)| {
        let byte = k[0];
        quote! {
            index.insert(#byte, &[#(#v),*] as &[u16]);
        }
    });

    let monogram_feature_index = quote! {
        pub const FEATURE_MONOGRAM_INDEX: once_cell::sync::Lazy<std::collections::HashMap<u8, &[u16]>> =
            once_cell::sync::Lazy::new(|| {
                let mut index = std::collections::HashMap::new();
                #(#monogram_index_insert_stmts)*
                index
            });
    };

    let bigram_index_insert_stmts = bigram_index.into_iter().map(|(k, v)| {
        let [b1, b2] = match &k[..] {
            &[b1, b2] => [b1, b2],
            _ => unreachable!(),
        };

        quote! {
            index.insert([#b1, #b2], &[#(#v),*] as &[u16]);
        }
    });

    let bigram_feature_index = quote! {
        pub const FEATURE_BIGRAM_INDEX: once_cell::sync::Lazy<std::collections::HashMap<[u8; 2], &[u16]>> =
            once_cell::sync::Lazy::new(|| {
                let mut index = std::collections::HashMap::new();
                #(#bigram_index_insert_stmts)*
                index
            });
    };

    let trigram_index_insert_stmts = trigram_index.into_iter().map(|(k, v)| {
        let [b1, b2, b3] = match &k[..] {
            &[b1, b2, b3] => [b1, b2, b3],
            _ => unreachable!(),
        };

        quote! {
            index.insert([#b1, #b2, #b3], &[#(#v),*] as &[u16]);
        }
    });

    let trigram_feature_index = quote! {
        pub const FEATURE_TRIGRAM_INDEX: once_cell::sync::Lazy<std::collections::HashMap<[u8; 3], &[u16]>> =
            once_cell::sync::Lazy::new(|| {
                let mut index = std::collections::HashMap::new();
                #(#trigram_index_insert_stmts)*
                index
            });
    };

    quote! {
        #versions
        #monogram_feature_index
        #bigram_feature_index
        #trigram_feature_index
    }
}

fn option_literal<T: ToTokens>(opt: &Option<T>) -> TokenStream {
    match opt {
        Some(lit) => quote! { Some(#lit) },
        None => quote! { None },
    }
}

fn add_feature_ngrams(
    n: usize,
    index: &mut BTreeMap<Vec<u8>, BTreeSet<u16>>,
    feature: &FeatureData,
    idx: u16,
) {
    let mut strings = vec![&feature.title];
    if let Some(f) = &feature.flag {
        strings.push(f);
    }
    strings.extend(feature.items.iter());

    for string in strings {
        for trigram in string.as_bytes().windows(n) {
            if trigram.iter().all(|&byte| byte.is_ascii_graphic() && byte != b'`') {
                index.entry(trigram.to_owned()).or_default().insert(idx);
            }
        }
    }
}
