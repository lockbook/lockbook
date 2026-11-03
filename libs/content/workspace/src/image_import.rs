//! Shared image imports for Markdown and canvas.
use crate::{
    file_cache::{FileCache, FilesExt},
    widgets::image_cache::ImageCache,
};
use chrono::DateTime;
use lb_rs::{
    Uuid,
    blocking::Lb,
    model::{file::File, file_metadata::FileType},
};
use web_time::{SystemTime, UNIX_EPOCH};

pub struct ImportedImage {
    pub file: File,
    pub destination: String,
    pub size: egui::Vec2,
}

/// Validate, import and refresh the common file cache before either host inserts a reference.
pub fn import(
    images: &ImageCache, core: &Lb, target: Uuid, name: Option<&str>, data: &[u8],
) -> Result<ImportedImage, String> {
    check_import_size(data)?;
    let size = crate::widgets::image_cache::image_dimensions(data)?;
    let name = image_import_name(name, data);
    let file = import_image_with_name(core, target, &name, data)?;
    let cache = FileCache::new(core).map_err(|e| e.to_string())?;
    let parent = cache
        .get_by_id(target)
        .ok_or("Import target is no longer available")?
        .parent;
    let destination = crate::file_cache::encode_link_path(&crate::file_cache::relative_path(
        &cache.path(parent),
        &cache.path(file.id),
    ));
    images.files().write().unwrap().replace(cache);
    Ok(ImportedImage { file, destination, size })
}

impl ImportedImage {
    pub fn markdown(&self) -> String {
        let label = self
            .file
            .name
            .replace('\\', "\\\\")
            .replace('[', "\\[")
            .replace(']', "\\]");
        format!("![{label}]({})", self.destination)
    }
}

fn image_extension(data: &[u8]) -> &'static str {
    if crate::widgets::image_cache::bytes_look_like_svg(data) {
        return "svg";
    }
    image::guess_format(data)
        .ok()
        .and_then(|fmt| fmt.extensions_str().first().copied())
        .unwrap_or("png")
}

pub const MAX_ATTACHMENT_SIZE_BYTES: usize = 25 * 1024 * 1024;

fn check_import_size(data: &[u8]) -> Result<(), String> {
    #[cfg(target_os = "android")]
    if data.len() > MAX_ATTACHMENT_SIZE_BYTES {
        return Err("Files larger than 25 MiB cannot be imported".to_owned());
    }
    let _ = data;
    Ok(())
}

// todo: use background thread
pub fn import_image(
    core: &Lb, file_id: Uuid, name: Option<&str>, data: &[u8],
) -> Result<File, String> {
    check_import_size(data)?;
    // Use the same decoder as display, including EXIF orientation and SVG.
    crate::widgets::image_cache::image_dimensions(data)?;
    let name = image_import_name(name, data);
    import_image_with_name(core, file_id, &name, data)
}

fn image_import_name(name: Option<&str>, data: &[u8]) -> String {
    let name = name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            let human_readable_time = DateTime::from_timestamp(time.as_secs() as _, 0)
                .expect("invalid system time")
                .format("%Y-%m-%d_%H-%M-%S");
            let file_extension = image_extension(data);
            format!("pasted_image_{human_readable_time}.{file_extension}")
        });
    name
}

/// Import image bytes next to a document, using a unique name in its `imports` folder.
fn import_image_with_name(
    core: &Lb, target: Uuid, name: &str, data: &[u8],
) -> Result<File, String> {
    let target_file = core.get_file_by_id(target).map_err(|e| e.to_string())?;
    let siblings = core
        .get_children(&target_file.parent)
        .map_err(|e| e.to_string())?;
    let imports = if let Some(folder) = siblings
        .iter()
        .find(|f| f.name == "imports" && f.is_folder())
    {
        folder.clone()
    } else {
        core.create_file("imports", &target_file.parent, FileType::Folder)
            .map_err(|e| e.to_string())?
    };
    let clean_name = name
        .chars()
        .map(|c| if c == '/' || c == '\\' || c.is_control() { '_' } else { c })
        .collect::<String>();
    let clean_name = clean_name.trim().trim_start_matches('.');
    let clean_name = if clean_name.is_empty() { "attachment" } else { clean_name };
    let clean_name = if !clean_name.contains('.') {
        let extension = image_extension(data);
        format!("{clean_name}.{extension}")
    } else {
        clean_name.to_owned()
    };
    let children = core.get_children(&imports.id).map_err(|e| e.to_string())?;
    let existing: std::collections::HashSet<_> = children.iter().map(|f| f.name.as_str()).collect();
    let stem = std::path::Path::new(&clean_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("attachment");
    let ext = std::path::Path::new(&clean_name)
        .extension()
        .and_then(|s| s.to_str());
    let chosen = (0..10000)
        .map(|i| {
            if i == 0 {
                clean_name.clone()
            } else if let Some(ext) = ext {
                format!("{stem} ({i}).{ext}")
            } else {
                format!("{stem} ({i})")
            }
        })
        .find(|candidate| !existing.contains(candidate.as_str()))
        .ok_or("Too many files with the same name")?;
    let file = core
        .create_file(&chosen, &imports.id, FileType::Document)
        .map_err(|e| e.to_string())?;
    if let Err(err) = core.write_document(file.id, data) {
        let _ = core.delete_file(&file.id);
        return Err(err.to_string());
    }
    Ok(file)
}

#[cfg(test)]
mod image_import_tests {
    use super::image_import_name;

    #[test]
    fn preserves_a_provided_image_name() {
        assert_eq!(image_import_name(Some(" photo.jpg "), &[]), "photo.jpg");
    }

    #[test]
    fn generates_a_timestamp_name_when_none_or_blank() {
        for name in [None, Some("   ")] {
            let generated = image_import_name(name, &[]);
            assert!(generated.starts_with("pasted_image_"));
            assert!(generated.ends_with(".png"));
        }
    }
}
