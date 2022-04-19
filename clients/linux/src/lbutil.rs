use std::fs;
use std::path::Path;
use std::sync::Arc;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

pub fn save_texture_to_png(
    api: &Arc<dyn lb::Api>, parent_id: lb::Uuid, texture: gdk::Texture,
) -> Result<lb::FileMetadata, String> {
    // There's a bit of a chicken and egg situation when it comes to naming a new file based on
    // its id. First, we'll create a new file with a random (temporary) name.
    let mut png_meta = api
        .create_file(&lb::Uuid::new_v4().to_string(), parent_id, lb::FileType::Document)
        .map_err(|err| format!("{:?}", err))?;

    // Then, the file is renamed to its id.
    let png_name = format!("{}.png", png_meta.id);
    api.rename_file(png_meta.id, &png_name)
        .map_err(|err| format!("{:?}", err))?;
    png_meta.decrypted_name = png_name;

    // Convert the texture to PNG bytes and write them to the newly created lockbook file.
    let png_data = texture.save_to_png_bytes();
    api.write_document(png_meta.id, &png_data)
        .map_err(|err| format!("{:?}", err))?;

    Ok(png_meta)
}

pub fn import_file(
    api: &Arc<dyn lb::Api>, disk_path: &Path, dest: lb::Uuid,
    new_file_tx: &glib::Sender<lb::FileMetadata>,
) -> Result<lb::FileMetadata, String> {
    if !disk_path.exists() {
        return Err(format!("invalid disk path {:?}", disk_path));
    }

    let disk_file_name = disk_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(format!("invalid disk path {:?}", disk_path))?;

    let file_type = match disk_path.is_file() {
        true => lb::FileType::Document,
        false => lb::FileType::Folder,
    };

    let file_name = {
        let siblings = api.children(dest).map_err(|e| e.0)?;
        lb::get_non_conflicting_name(&siblings, disk_file_name)
    };

    let file_meta = api
        .create_file(&file_name, dest, file_type)
        .map_err(|e| format!("{:?}", e))?;

    new_file_tx.send(file_meta.clone()).unwrap();

    if file_type == lb::FileType::Document {
        let content = fs::read(&disk_path).map_err(|e| format!("{:?}", e))?;
        api.write_document(file_meta.id, &content)
            .map_err(|e| format!("{:?}", e))?;
    } else {
        let entries = fs::read_dir(disk_path).map_err(|e| format!("{:?}", e))?;
        for entry in entries {
            let child_path = entry.map_err(|e| format!("{:?}", e))?.path();
            import_file(api, &child_path, file_meta.id, new_file_tx)
                .map_err(|e| format!("{:?}", e))?;
        }
    }

    Ok(file_meta)
}
