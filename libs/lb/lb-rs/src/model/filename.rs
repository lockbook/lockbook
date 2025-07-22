use crate::model::file::File;

pub const MAX_FILENAME_LENGTH: usize = 230;
pub const MAX_ENCRYPTED_FILENAME_LENGTH: usize = MAX_FILENAME_LENGTH + 24;

#[derive(Debug, PartialEq, Eq)]
pub enum DocumentType {
    Text,
    Drawing,
    Other,
}

// todo: be more exhaustive
impl DocumentType {
    pub fn from_file_name_using_extension(name: &str) -> DocumentType {
        match name.split('.').next_back() {
            Some("md") | Some("txt") => DocumentType::Text,
            Some("svg") => DocumentType::Drawing,
            _ => DocumentType::Other,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct NameComponents {
    pub name: String,
    pub variant: Option<usize>,
    pub extension: Option<String>,
}

impl NameComponents {
    pub fn from(file_name: &str) -> NameComponents {
        let extension_location = file_name.rfind('.').and_then(|location| {
            if location == file_name.len() - 1 {
                None
            } else {
                Some(location)
            }
        });

        let name_with_variant = match extension_location {
            Some(location) => &file_name[..location],
            None => file_name,
        };

        let mut variant_location = name_with_variant.rfind('-');

        let variant = variant_location
            .map(|location| name_with_variant[location + 1..].to_string())
            .and_then(|maybe_variant| maybe_variant.parse::<usize>().ok());

        if variant.is_none() {
            variant_location = None
        }

        let name = {
            let name_right_bound =
                variant_location.unwrap_or_else(|| extension_location.unwrap_or(file_name.len()));
            file_name[0..name_right_bound].to_string()
        };

        let extension = extension_location.map(|location| file_name[location + 1..].to_string());

        NameComponents { name, variant, extension }
    }

    pub fn generate_next(&self) -> NameComponents {
        self.generate_incremented(1)
    }

    pub fn generate_incremented(&self, n: usize) -> NameComponents {
        let mut next = self.clone();
        next.variant = Some(self.variant.unwrap_or(0) + n);
        next
    }

    pub fn next_in_children(&self, children: Vec<File>) -> NameComponents {
        let mut next = self.clone();

        let mut children: Vec<NameComponents> = children
            .iter()
            .filter_map(|f| {
                let nc = NameComponents::from(&f.name);
                if nc.name == next.name && nc.extension == next.extension {
                    Some(nc)
                } else {
                    None
                }
            })
            .collect();

        children.sort_by(|a, b| {
            a.variant
                .unwrap_or_default()
                .cmp(&b.variant.unwrap_or_default())
        });

        for (i, child) in children.iter().enumerate() {
            if let Some(variant) = child.variant {
                if i == 0 && variant > 0 {
                    break; //insert self without an increment
                }
            }

            if let Some(curr) = children.get(i + 1) {
                if curr.variant.unwrap_or_default() != child.variant.unwrap_or_default() + 1 {
                    next = child.generate_next();
                    break;
                }
            } else {
                next = child.generate_next();
            }
        }

        next
    }

    pub fn to_name(&self) -> String {
        match (&self.variant, &self.extension) {
            (Some(variant), Some(extension)) => format!("{}-{}.{}", self.name, variant, extension),
            (Some(variant), None) => format!("{}-{}", self.name, variant),
            (None, Some(extension)) => format!("{}.{}", self.name, extension),
            (None, None) => self.name.to_string(),
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::filename::NameComponents;
    use crate::model::{file::File, file_metadata::FileType};

    fn from_components(
        name: &str, variant: Option<usize>, extension: Option<&str>,
    ) -> NameComponents {
        NameComponents {
            name: name.to_string(),
            variant,
            extension: extension.map(|str| str.to_string()),
        }
    }

    #[test]
    fn test_name_components() {
        assert_eq!(NameComponents::from("test-1.md"), from_components("test", Some(1), Some("md")));
        assert_eq!(NameComponents::from("test-.md"), from_components("test-", None, Some("md")));
        assert_eq!(NameComponents::from(".md"), from_components("", None, Some("md")));
        assert_eq!(NameComponents::from(""), from_components("", None, None));
        assert_eq!(
            NameComponents::from("test-file.md"),
            from_components("test-file", None, Some("md"))
        );
        assert_eq!(
            NameComponents::from("test-file-1.md"),
            from_components("test-file", Some(1), Some("md"))
        );
        assert_eq!(
            NameComponents::from("test-file-1.md."),
            from_components("test-file-1.md.", None, None)
        );
        assert_eq!(
            NameComponents::from("test-file-1.m"),
            from_components("test-file", Some(1), Some("m"))
        );
        assert_eq!(
            NameComponents::from("test-file-100.m"),
            from_components("test-file", Some(100), Some("m"))
        );
        assert_eq!(
            NameComponents::from("test-file--100.m"),
            from_components("test-file-", Some(100), Some("m"))
        );
        assert_eq!(
            NameComponents::from("test-file-.-100.m"),
            from_components("test-file-.", Some(100), Some("m"))
        );
        assert_eq!(NameComponents::from("."), from_components(".", None, None));
        assert_eq!(NameComponents::from("-1."), from_components("-1.", None, None));
        assert_eq!(NameComponents::from("-1."), from_components("-1.", None, None));
        assert_eq!(NameComponents::from("test"), from_components("test", None, None));
        assert_eq!(NameComponents::from("test-32"), from_components("test", Some(32), None));
    }

    fn assert_symmetry(name: &str) {
        assert_eq!(NameComponents::from(name).to_name(), name);
    }

    #[test]
    fn test_back_to_name() {
        assert_symmetry("test-1.md");
        assert_symmetry("test-.md");
        assert_symmetry(".md");
        assert_symmetry("");
        assert_symmetry("test-file.md");
        assert_symmetry("test-file-1.md");
        assert_symmetry("test-file-1.md.");
        assert_symmetry("test-file-1.m");
        assert_symmetry("test-file-100.m");
        assert_symmetry("test-file--100.m");
        assert_symmetry("test-file-.-100.m");
        assert_symmetry(".");
        assert_symmetry("-1.");
        assert_symmetry("-1.");
        assert_symmetry("test");
        assert_symmetry("test-32");
    }

    #[test]
    fn test_next_variant() {
        assert_eq!(NameComponents::from("test.md").generate_next().to_name(), "test-1.md");
        assert_eq!(NameComponents::from("test-2.md").generate_next().to_name(), "test-3.md");
    }
    #[test]
    fn test_next_in_children() {
        let children = new_files(vec!["untitled.md", "untitled-1.md", "untitled-2.md"]);
        let next = NameComponents::from("untitled.md").next_in_children(children);
        assert_eq!(next.to_name(), "untitled-3.md");

        let children =
            new_files(vec!["untitled-1.md", "untitled-2.md", "untitled-3.md", "untitled-4.md"]);
        let next = NameComponents::from("untitled.md").next_in_children(children);
        assert_eq!(next.to_name(), "untitled.md");

        let children = new_files(vec!["untitled.md", "untitled-2.md", "untitled-3.md"]);
        let next = NameComponents::from("untitled.md").next_in_children(children);
        assert_eq!(next.to_name(), "untitled-1.md");
    }

    fn new_files(names: Vec<&str>) -> Vec<File> {
        names
            .iter()
            .map(|&name| File {
                id: Uuid::default(),
                parent: Uuid::default(),
                name: name.to_string(),
                file_type: FileType::Document,
                last_modified: u64::default(),
                last_modified_by: String::default(),
                shares: vec![],
            })
            .collect()
    }
}
