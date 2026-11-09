use crate::model::file::File;

pub const MAX_FILENAME_LENGTH: usize = 230;
pub const MAX_ENCRYPTED_FILENAME_LENGTH: usize = MAX_FILENAME_LENGTH + 24;

#[derive(Debug, PartialEq, Eq)]
pub enum DocumentType {
    Text,
    Drawing,
    Chat,
    Json,
    Other,
}

// todo: be more exhaustive
impl DocumentType {
    pub fn from_file_name_using_extension(name: &str) -> DocumentType {
        match name.split('.').next_back() {
            Some("md") | Some("txt") => DocumentType::Text,
            Some("svg") => DocumentType::Drawing,
            Some("chat") => DocumentType::Chat,
            Some("json") => DocumentType::Json,
            _ => DocumentType::Other,
        }
    }
}

/// A file name split to pick a free name beside it, as Finder does: copies are `name copy.md`
/// then `name copy 2.md`, and other free names are `name 2.md`. Only the copy marker is read
/// back out of a name, so dates and numbers people type stay as written.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct NameComponents {
    name: String,
    /// `Some(1)` is ` copy`, `Some(n)` is ` copy n`
    copy: Option<usize>,
    extension: Option<String>,
}

impl NameComponents {
    /// A document's name. Its extension follows the last dot when that text has a letter and no
    /// whitespace and the dot doesn't start the name, so `Dr. Smith`, `10.5 notes` and
    /// `.gitignore` have none.
    pub fn from(file_name: &str) -> NameComponents {
        match file_name.rsplit_once('.') {
            Some((stem, extension)) if !stem.is_empty() && is_extension(extension) => {
                Self::split(stem, Some(extension.to_string()))
            }
            _ => Self::split(file_name, None),
        }
    }

    /// A folder's name, which has no extension.
    pub fn folder(file_name: &str) -> NameComponents {
        Self::split(file_name, None)
    }

    fn split(stem: &str, extension: Option<String>) -> NameComponents {
        let numbered_copy = stem.rsplit_once(' ').and_then(|(rest, digits)| {
            let name = rest.strip_suffix(" copy")?;
            let n = digits.parse::<usize>().ok()?;
            (n >= 2 && n.to_string() == digits).then_some((name, Some(n)))
        });
        let (name, copy) = numbered_copy
            .or_else(|| stem.strip_suffix(" copy").map(|name| (name, Some(1))))
            .unwrap_or((stem, None));
        NameComponents { name: name.to_string(), copy, extension }
    }

    /// The `n`th copy of this name, counting on from the copy it already is.
    pub fn copy(&self, n: usize) -> String {
        NameComponents { copy: Some(self.copy.unwrap_or(0) + n), ..self.clone() }.to_name()
    }

    /// This name with ` n` after it, or the name itself for 1.
    pub fn numbered(&self, n: usize) -> String {
        let number = if n == 1 { String::new() } else { format!(" {n}") };
        format!("{}{}{number}{}", self.name, self.copy_marker(), self.dot_extension())
    }

    /// The first copy whose name no child has.
    pub fn free_copy_in(&self, children: &[File]) -> String {
        first_free(children, |n| self.copy(n))
    }

    /// The first numbered name no child has, starting from this name itself.
    pub fn free_number_in(&self, children: &[File]) -> String {
        first_free(children, |n| self.numbered(n))
    }

    pub fn to_name(&self) -> String {
        self.numbered(1)
    }

    fn copy_marker(&self) -> String {
        match self.copy {
            None => String::new(),
            Some(1) => " copy".to_string(),
            Some(n) => format!(" copy {n}"),
        }
    }

    fn dot_extension(&self) -> String {
        self.extension
            .as_ref()
            .map(|extension| format!(".{extension}"))
            .unwrap_or_default()
    }
}

fn is_extension(text: &str) -> bool {
    text.chars().any(char::is_alphabetic) && !text.chars().any(char::is_whitespace)
}

fn first_free(children: &[File], name: impl Fn(usize) -> String) -> String {
    let mut n = 1;
    loop {
        let candidate = name(n);
        if children.iter().all(|child| child.name != candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::filename::NameComponents;

    #[test]
    fn names_round_trip() {
        for name in [
            "",
            ".",
            ".md",
            ".gitignore",
            "test",
            "test.md",
            "test-1.md",
            "test-32",
            "test.",
            "2026-11-07.md",
            "2026-11-07",
            "Dr. Smith",
            "10.5 release notes",
            "archive.tar.gz",
            "notes copy.md",
            "notes copy 2.md",
            "notes copy 1.md",
            "notes copy 02.md",
            " copy",
        ] {
            assert_eq!(NameComponents::from(name).to_name(), name);
            assert_eq!(NameComponents::folder(name).to_name(), name);
        }
    }

    #[test]
    fn copies_and_numbers() {
        for (name, copy, copy_2, numbered) in [
            ("2026-11-07.md", "2026-11-07 copy.md", "2026-11-07 copy 2.md", "2026-11-07 2.md"),
            ("chapter-3.md", "chapter-3 copy.md", "chapter-3 copy 2.md", "chapter-3 2.md"),
            ("notes 2.md", "notes 2 copy.md", "notes 2 copy 2.md", "notes 2 2.md"),
            ("notes copy.md", "notes copy 2.md", "notes copy 3.md", "notes copy 2.md"),
            ("Dr. Smith", "Dr. Smith copy", "Dr. Smith copy 2", "Dr. Smith 2"),
            ("Q3.2026", "Q3.2026 copy", "Q3.2026 copy 2", "Q3.2026 2"),
            (".gitignore", ".gitignore copy", ".gitignore copy 2", ".gitignore 2"),
        ] {
            let components = NameComponents::from(name);
            assert_eq!(components.copy(1), copy);
            assert_eq!(components.copy(2), copy_2);
            assert_eq!(components.numbered(2), numbered);
        }
        assert_eq!(NameComponents::folder("Project.Alpha").numbered(2), "Project.Alpha 2");
    }
}
