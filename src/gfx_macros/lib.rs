// Copyright 2014 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![feature(macro_rules, plugin_registrar, quote)]

//! Macro extensions crate.
//! Implements `shaders!` macro as well as `#[shader_param]` and
//! `#[vertex_format]` attributes.

extern crate rustc;
extern crate syntax;

use syntax::{ast, attr, ext, codemap};
use syntax::ext::build::AstBuilder;
use syntax::parse::token;
use syntax::fold::Folder;
use syntax::ptr::P;

pub mod shader_param;
pub mod vertex_format;

/// Entry point for the plugin phase
#[plugin_registrar]
pub fn registrar(reg: &mut rustc::plugin::Registry) {
    use syntax::parse::token::intern;
    use syntax::ext::base;
    // Register the `#[shader_param]` attribute.
    reg.register_syntax_extension(intern("shader_param"),
        base::Decorator(box shader_param::expand));
    // Register the `#[vertex_format]` attribute.
    reg.register_syntax_extension(intern("vertex_format"),
        base::Decorator(box vertex_format::expand));
}

/// Scan through the field's attributes and extract the field vertex name. If
/// multiple names are found, use the first name and emit a warning.
fn find_name(cx: &mut ext::base::ExtCtxt, span: codemap::Span,
             attributes: &[ast::Attribute]) -> Option<token::InternedString> {
    attributes.iter().fold(None, |name, attribute| {
        match attribute.node.value.node {
            ast::MetaNameValue(ref attr_name, ref attr_value) => {
                match (attr_name.get(), &attr_value.node) {
                    ("name", &ast::LitStr(ref new_name, _)) => {
                        attr::mark_used(attribute);
                        name.map_or(Some(new_name.clone()), |name| {
                            cx.span_warn(span, format!(
                                "Extra field name detected: {} - \
                                ignoring in favour of: {}", new_name, name
                            ).as_slice());
                            None
                        })
                    }
                    _ => None,
                }
            }
            _ => name,
        }
    })
}

/// Marker string to base the unique identifier generated by `extern_crate_hack()` on
static EXTERN_CRATE_HACK: &'static str = "__gfx_extern_crate_hack";

/// Inserts a module with a unique identifier that reexports
/// The `gfx` crate, and returns that identifier
fn extern_crate_hack(context: &mut ext::base::ExtCtxt,
                     span: codemap::Span,
                     push: |P<ast::Item>|) -> ast::Ident {
    let extern_crate_hack = token::gensym_ident(EXTERN_CRATE_HACK);
    // mod $EXTERN_CRATE_HACK {
    //     extern crate gfx_ = "gfx";
    //     pub use gfx_ as gfx;
    // }
    let item = context.item_mod(
        span,
        span,
        extern_crate_hack,
        vec![],
        vec![
            ast::ViewItem {
                span: span,
                vis: ast::Inherited,
                attrs: vec![],
                node: ast::ViewItemExternCrate(
                    context.ident_of("gfx_"),
                    Some((
                        token::InternedString::new("gfx"),
                        ast::CookedStr
                    )),
                    ast::DUMMY_NODE_ID
                )
            },
            context.view_use_simple_(
                span,
                ast::Public,
                context.ident_of("gfx"),
                context.path(span, vec![
                    context.ident_of("self"),
                    context.ident_of("gfx_")
                ])
            )
        ],
        vec![]
    );
    push(item);
    extern_crate_hack
}

/// This Folder gets used to fixup all paths generated by the
/// deriving trait impl to point to the unique module
/// containing the `gfx` reexport.
struct ExternCrateHackFolder {
    path_root: ast::Ident
}

impl Folder for ExternCrateHackFolder {
    fn fold_path(&mut self, p: ast::Path) -> ast::Path {
        let p = syntax::fold::noop_fold_path(p, self);
        let needs_fix = p.segments.as_slice().get(0)
                         .map(|s| s.identifier.as_str() == EXTERN_CRATE_HACK)
                         .unwrap_or(false);
        let needs_fix_self = p.segments.as_slice().get(0)
                              .map(|s| s.identifier.as_str() == "self")
                              .unwrap_or(false) &&
                             p.segments.as_slice().get(1)
                              .map(|s| s.identifier.as_str() == EXTERN_CRATE_HACK)
                              .unwrap_or(false);

        if needs_fix {
            let mut p = p.clone();
            p.segments[0].identifier = self.path_root;
            p.global = false;
            p
        } else if needs_fix_self {
            let mut p = p.clone();
            p.segments[1].identifier = self.path_root;
            p.global = false;
            p
        } else {
            p
        }

    }
}

/// Simply applies the `ExternCrateHackFolder`
fn fixup_extern_crate_paths(item: P<ast::Item>, path_root: ast::Ident) -> P<ast::Item> {
    ExternCrateHackFolder {
        path_root: path_root
    }.fold_item(item).into_iter().next().unwrap()
}

// The `gfx` reexport module here does not need a unique name,
// as it gets inserted in a new block and thus doesn't conflict with
// any names outside its lexical scope.
#[macro_export]
macro_rules! shaders {
    (GLSL_120: $v:expr $($t:tt)*) => {
        {
            mod __gfx_extern_crate_hack {
                extern crate "gfx" as gfx_;
                pub use self::gfx_ as gfx;
            }
            __gfx_extern_crate_hack::gfx::ShaderSource {
                glsl_120: Some($v),
                ..shaders!($($t)*)
            }
        }
    };
    (GLSL_130: $v:expr $($t:tt)*) => {
        {
            mod __gfx_extern_crate_hack {
                extern crate "gfx" as gfx_;
                pub use self::gfx_ as gfx;
            }
            __gfx_extern_crate_hack::gfx::ShaderSource {
                glsl_130: Some($v),
                ..shaders!($($t)*)
            }
        }
    };
    (GLSL_140: $v:expr $($t:tt)*) => {
        {
            mod __gfx_extern_crate_hack {
                extern crate "gfx" as gfx_;
                pub use self::gfx_ as gfx;
            }
            __gfx_extern_crate_hack::gfx::ShaderSource {
                glsl_140: Some($v),
                ..shaders!($($t)*)
            }
        }
    };
    (GLSL_150: $v:expr $($t:tt)*) => {
        {
            mod __gfx_extern_crate_hack {
                extern crate "gfx" as gfx_;
                pub use self::gfx_ as gfx;
            }
            __gfx_extern_crate_hack::gfx::ShaderSource {
                glsl_150: Some($v),
                ..shaders!($($t)*)
            }
        }
    };
    (TARGETS: $v:expr $($t:tt)*) => {
        {
            mod __gfx_extern_crate_hack {
                extern crate "gfx" as gfx_;
                pub use self::gfx_ as gfx;
            }
            __gfx_extern_crate_hack::gfx::ShaderSource {
                targets: Some($v),
                ..shaders!($($t)*)
            }
        }
    };
    () => {
        {
            mod __gfx_extern_crate_hack {
                extern crate "gfx" as gfx_;
                pub use self::gfx_ as gfx;
            }
            __gfx_extern_crate_hack::gfx::ShaderSource {
                glsl_120: None,
                glsl_130: None,
                glsl_140: None,
                glsl_150: None,
                targets: None,
            }
        }
    }
}
