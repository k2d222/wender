use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::Write,
    fs::{self, File},
    path::{Path, PathBuf},
};

use itertools::Itertools;
use regex::{Captures, Regex};
use thiserror::Error;

/// a straightforward wgsl preprocessor.

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to read wgsl file: {0}")]
    IOError(PathBuf),
}

pub struct Context {
    pub main: PathBuf,
    pub constants: HashMap<String, String>,
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
        .map(|(k, v)| format!("const {k} = {v};\n"))
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

    println!("{source}");
}
