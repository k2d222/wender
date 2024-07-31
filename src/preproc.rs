use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::Write,
    fs::{self, File},
    path::{Path, PathBuf},
};

use itertools::Itertools;
use naga_oil::compose::{
    self, ComposableModuleDescriptor, Composer, ComposerError, NagaModuleDescriptor,
    ShaderDefValue, ShaderType,
};
use regex::{Captures, Regex};
use thiserror::Error;
use wgpu::naga::{
    self,
    front::wgsl,
    valid::{Capabilities, ShaderStages},
};

/// a straightforward wgsl preprocessor.

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to read `{0}`")]
    IOError(PathBuf),
    #[error("while composing `{0}`: {1}")]
    ComposerError(PathBuf, String, ComposerError),
}

pub struct Context<'a> {
    pub main: &'a Path,
    pub constants: &'a HashMap<String, f64>,
}

pub fn preprocess_shader(context: &Context) -> Result<naga::Module, Error> {
    enum TmpError {
        Processed(Error),
        Unprocessed(PathBuf, ComposerError),
    }
    fn rec_preproc(
        composer: &mut Composer,
        path: &Path,
        defs: &HashMap<String, ShaderDefValue>,
    ) -> Result<(), TmpError> {
        let mod_name = format!("\"{}\"", path.file_name().unwrap().to_string_lossy());

        if composer.contains_module(&mod_name) {
            return Ok(());
        }

        let source = fs::read_to_string(path)
            .map_err(|_| TmpError::Processed(Error::IOError(path.to_owned())))?;
        let (name, imports, defines) = naga_oil::compose::get_preprocessor_data(&source);

        for import in imports.iter() {
            if import.import.starts_with('"') && import.import.ends_with('"') {
                let mut path = path.parent().unwrap().to_path_buf();
                path.push(&import.import[1..import.import.len() - 1]);
                rec_preproc(composer, &path, defs)?;
            }
        }

        let module = composer
            .add_composable_module(ComposableModuleDescriptor {
                source: &source,
                file_path: path.to_str().unwrap(),
                language: compose::ShaderLanguage::Wgsl,
                as_name: Some(mod_name),
                additional_imports: &[],
                shader_defs: defs.clone(),
            })
            .map_err(|e| TmpError::Unprocessed(path.to_owned(), e))?;

        Ok(())
    }

    let mut composer =
        Composer::default().with_capabilities(Capabilities::all(), ShaderStages::all());

    let defs = HashMap::from_iter(
        context
            .constants
            .iter()
            .map(|(k, v)| (k.to_owned(), ShaderDefValue::UInt(*v as u32))),
    );

    let source =
        fs::read_to_string(&context.main).map_err(|_| Error::IOError(context.main.to_owned()))?;

    let (name, imports, defines) = naga_oil::compose::get_preprocessor_data(&source);

    // oh don't mind me I'm just fighting the borrow checker here.
    // this is a for loop with early return on error.
    let err = imports.iter().find_map(|import| {
        if import.import.starts_with('"') && import.import.ends_with('"') {
            let mut path = context.main.parent().unwrap().to_path_buf();
            path.push(&import.import[1..import.import.len() - 1]);
            let res = rec_preproc(&mut composer, &path, &defs);
            res.err()
        } else {
            None
        }
    });

    match err {
        Some(e) => match e {
            TmpError::Processed(e) => return Err(e),
            TmpError::Unprocessed(path, e) => {
                return Err(Error::ComposerError(path, e.emit_to_string(&composer), e));
            }
        },
        None => (),
    }

    let module = composer
        .make_naga_module(NagaModuleDescriptor {
            source: &source,
            file_path: context.main.to_str().unwrap(),
            shader_type: ShaderType::Wgsl,
            shader_defs: defs,
            additional_imports: &[],
        })
        .map_err(|e| {
            Error::ComposerError(context.main.to_owned(), e.emit_to_string(&composer), e)
        })?;

    Ok(module)
}

pub fn build_shader(context: &Context) -> Result<String, Error> {
    fn rec_preprocess(path: &Path, included_files: &mut Vec<PathBuf>) -> Result<String, Error> {
        // avoid multiple inclusions
        // TODO: canonicalize path
        {
            let path_owned = path.to_owned();
            if included_files.contains(&path_owned) {
                return Ok(format!("// preproc: skipped {}\n", path.display()));
            }
            included_files.push(path_owned);
        }

        let source = fs::read_to_string(path).map_err(|_| Error::IOError(path.to_owned()))?;
        let re = Regex::new(r#"(?m)^(?:// )?preproc_include\(([^"]+?)\)"#).unwrap();
        let mut expanded_source = source.clone();

        for captures in re.captures_iter(&source) {
            let filename = captures.get(1).unwrap().as_str();
            let mut path = path.parent().unwrap().to_owned();
            path.push(filename);
            let include_source = rec_preprocess(&path, included_files)?;
            let include_source = format!(
                "// preproc: begin \"{1}\"\n{0}\n// preproc: end \"{1}\"\n",
                include_source,
                path.display()
            );

            let cap = captures.get(0).unwrap();
            expanded_source.replace_range(cap.range(), &include_source);
        }

        Ok(expanded_source)
    }

    let source = rec_preprocess(&context.main, &mut vec![])?;

    let constants = context
        .constants
        .iter()
        .map(|(k, v)| format!("const {k} = {v}u;\n")) // BUG: It would be great to have AbstractInt type there, but naga is not there yet.
        .format("\n");

    let source = format!(
        "//////////////////////////////\n\
         // PREPROCESSED WGSL SHADER //\n\
         //////////////////////////////\n\
         \n\
         // this wgsl shader was preprocessed by {}.\n\
         \n\
         // preproc: constants\n\
         {}\n\
         \n\
         // preproc: main \"{}\"\n\
         {}",
        module_path!(),
        constants,
        context.main.display(),
        source,
    );

    Ok(source)

    // let mut module = wgpu::naga::front::wgsl::parse_str(&source).map_err(Error::NagaError)?;
    // for constant in module.constants.iter() {
    //     println!("constant: {constant:?}");
    // }

    // Ok(module)
}
