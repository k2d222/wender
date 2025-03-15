use std::path::Path;

use wesl::*;
use wgpu::naga::{self};

/// a straightforward wgsl preprocessor.

pub struct Context<'a> {
    pub main: &'a Path,
    pub constants: &'a syntax::TranslationUnit,
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    CompileError(#[from] wesl::Error),
    #[error("naga parse error: {0}")]
    NagaError(#[from] naga::front::wgsl::ParseError),
}

pub fn compile_shader(context: &Context) -> Result<naga::Module, Error> {
    let base = context.main.parent().expect("shader not found");
    let name = context.main.file_name().expect("shader not found");

    let mut file_resolver = FileResolver::new(base);
    file_resolver.set_extension("wgsl");
    let mut virt_resolver = VirtualResolver::new();
    virt_resolver.add_module("", context.constants.to_string().into());
    let mut router = Router::new();
    router.mount_fallback_resolver(file_resolver);
    router.mount_resolver("/constants", virt_resolver);

    let source = wesl::Wesl::new(".")
        .set_custom_resolver(router)
        .set_options(CompileOptions {
            strip: false,
            lower: false,
            // lazy: false,
            ..Default::default()
        })
        .compile(name)?
        .to_string();

    println!("shader `{}`", name.to_string_lossy());
    println!("{source}");
    let module = wgpu::naga::front::wgsl::parse_str(&source)
        .inspect_err(|e| e.emit_to_stderr(&source))
        .map_err(Error::NagaError)?;
    Ok(module)
}
