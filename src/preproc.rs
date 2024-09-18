use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use itertools::Itertools;
use weslc::*;
use wgpu::naga::{self};

/// a straightforward wgsl preprocessor.

pub struct Context<'a> {
    pub main: &'a Path,
    pub constants: &'a syntax::TranslationUnit,
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
    #[error("input file not found")]
    FileNotFound,
    #[error("{0}")]
    CompileError(#[from] weslc::Error),
    #[error("naga parse error: {0}")]
    NagaError(#[from] naga::front::wgsl::ParseError),
}

pub fn compile_shader(context: &Context) -> Result<naga::Module, Error> {
    let base = context
        .main
        .parent()
        .ok_or(Error::FileNotFound)?
        .to_path_buf();
    let name = PathBuf::from(context.main.file_name().ok_or(Error::FileNotFound)?);

    let file_resolver = FileResolver::new(base);
    let mut virtual_resolver = VirtualFileResolver::new();
    virtual_resolver.add_file(Path::new("").to_path_buf(), context.constants.to_string())?;
    let mut resolver = DispatchResolver::new();
    resolver.mount_fallback_resolver(Box::new(file_resolver));
    resolver.mount_resolver(
        Path::new("constants").to_path_buf(),
        Box::new(virtual_resolver),
    );

    let entrypoint: Resource = name.into();

    let mangler = MANGLER_ESCAPE;

    let compile_options = CompileOptions {
        ..Default::default()
    };

    let wgsl = weslc::compile(&entrypoint, resolver, &mangler, &compile_options)?;
    let source = wgsl.to_string();
    println!("{source}");
    let module = wgpu::naga::front::wgsl::parse_str(&source)
        .inspect_err(|e| e.emit_to_stderr(&source))
        .map_err(Error::NagaError)?;
    Ok(module)
}

// pub fn build_shader(context: &Context) -> Result<String, Error> {
//     fn rec_preprocess(path: &Path, included_files: &mut Vec<PathBuf>) -> Result<String, Error> {
//         // avoid multiple inclusions
//         // TODO: canonicalize path
//         {
//             let path_owned = path.to_owned();
//             if included_files.contains(&path_owned) {
//                 return Ok(format!("// preproc: skipped {}\n", path.display()));
//             }
//             included_files.push(path_owned);
//         }

//         let source = fs::read_to_string(path).map_err(|_| Error::IOError(path.to_owned()))?;
//         let re = Regex::new(r#"(?m)^(?:// )?preproc_include\(([^"]+?)\)"#).unwrap();
//         let mut expanded_source = source.clone();

//         for captures in re.captures_iter(&source) {
//             let filename = captures.get(1).unwrap().as_str();
//             let mut path = path.parent().unwrap().to_owned();
//             path.push(filename);
//             let include_source = rec_preprocess(&path, included_files)?;
//             let include_source = format!(
//                 "// preproc: begin \"{1}\"\n{0}\n// preproc: end \"{1}\"\n",
//                 include_source,
//                 path.display()
//             );

//             let cap = captures.get(0).unwrap();
//             expanded_source.replace_range(cap.range(), &include_source);
//         }

//         Ok(expanded_source)
//     }

//     let source = rec_preprocess(&context.main, &mut vec![])?;

//     let constants = context
//         .constants
//         .iter()
//         .map(|(k, v)| format!("const {k} = {v}u;\n")) // BUG: It would be great to have AbstractInt type there, but naga is not there yet.
//         .format("\n");

//     let source = format!(
//         "//////////////////////////////\n\
//          // PREPROCESSED WGSL SHADER //\n\
//          //////////////////////////////\n\
//          \n\
//          // this wgsl shader was preprocessed by {}.\n\
//          \n\
//          // preproc: constants\n\
//          {}\n\
//          \n\
//          // preproc: main \"{}\"\n\
//          {}",
//         module_path!(),
//         constants,
//         context.main.display(),
//         source,
//     );

//     Ok(source)

//     // let mut module = wgpu::naga::front::wgsl::parse_str(&source).map_err(Error::NagaError)?;
//     // for constant in module.constants.iter() {
//     //     println!("constant: {constant:?}");
//     // }

//     // Ok(module)
// }
