use itertools::Itertools;
use regex::{Captures, Regex};

fn main() {
    let source = include_str!("../../src/shader.wgsl");

    let pattern = r#"#\[recursive (\d+)\]\s*fn(\s+)([\w_\d]+)([\s\S]+?\n\})"#;
    let re = Regex::new(pattern).unwrap();

    let processed = re.replace_all(source, |captures: &Captures| -> String {
        let rec_iters = captures.get(1).unwrap().as_str().parse::<u32>().unwrap();
        let s1 = captures.get(2).unwrap().as_str();
        let fn_name = captures.get(3).unwrap().as_str();
        let content = captures.get(4).unwrap().as_str();

        (1..rec_iters)
            .map(|n| {
                let content = content.replace(fn_name, &format!("{fn_name}_{}", n - 1));
                format!("fn{s1}{fn_name}_{n}{content}")
            })
            .format("\n\n")
            .to_string()
    });

    println!("{processed}");
}
