use harper_core::linting::{Lint, LintGroup, LintGroupConfig, Linter};
use harper_core::{Document, FstDictionary};

#[derive(Default)]
pub struct Grammar {
    pub lints: Vec<Lint>,
}

pub fn calc(text: &str) -> Grammar {
    let dict = FstDictionary::curated();
    let document = Document::new_markdown(text, &dict);

    let mut linter = LintGroup::new(LintGroupConfig { ..Default::default() }, dict);

    let start = std::time::Instant::now();
    let mut lints = linter.lint(&document);
    println!("linting took {:?}", start.elapsed()); // prints values like 1.2s for ~35kb of text

    // lints must be non-overlapping and sorted (for now)
    lints.sort_by_key(|l| l.span.start);
    let mut current_lint = 0;
    let mut i = 0;
    loop {
        if i >= lints.len() {
            break;
        }
        if i != 0 && lints[i].span.start < lints[current_lint].span.end {
            lints.remove(i);
        } else {
            current_lint = i;
            i += 1;
        }
    }

    Grammar { lints }
}
