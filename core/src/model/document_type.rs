pub enum DocumentType {
    Text,
    Drawing,
    Other,
}

// todo: be more exhaustive
impl DocumentType {
    pub fn from_file_name_using_extension(name: &str) -> DocumentType {
        match name.split(".").last() {
            Some(".md") => DocumentType::Text,
            Some(".draw") => DocumentType::Drawing,
            _ => DocumentType::Other,
        }
    }
}
