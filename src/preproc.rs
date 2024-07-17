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

fn replace_all<E>(
    re: &Regex,
    haystack: &str,
    replacement: impl Fn(&Captures) -> Result<String, E>,
) -> Result<String, E> {
    let mut new = String::with_capacity(haystack.len());
    let mut last_match = 0;
    for caps in re.captures_iter(haystack) {
        let m = caps.get(0).unwrap();
        new.push_str(&haystack[last_match..m.start()]);
        new.push_str(&replacement(&caps)?);
        last_match = m.end();
    }
    new.push_str(&haystack[last_match..]);
    Ok(new)
}

pub struct Context {
    pub main: PathBuf,
    pub constants: HashMap<String, String>,
}

pub fn build_shader(context: &Context) -> Result<String, Error> {
    let source =
        fs::read_to_string(&context.main).map_err(|_| Error::IOError(context.main.clone()))?;

    let re = Regex::new(r#"// preproc::include\s*"([^"]+)"\s*"#).unwrap();
    let source = replace_all(&re, &source, |captures| {
        let filename = captures.get(1).unwrap().as_str();
        let mut path = context.main.parent().unwrap().to_owned();
        path.push(filename);
        let source = fs::read_to_string(&path).map_err(|_| Error::IOError(path))?;
        let source = format!("{}\n{}", captures.get(0).unwrap().as_str(), source);
        Ok(source)
    })?;

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
         // preproc::constants\n\
         {}\n\
         \n\
         // preproc::main \"{}\"\n\
         {}",
        module_path!(),
        constants,
        context.main.display(),
        source,
    );
    Ok(source)
}
