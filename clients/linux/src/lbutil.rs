use std::env;
use std::fs;
use std::path::Path;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

pub fn data_dir() -> String {
    const ERR_MSG: &str = "Unable to determine a Lockbook data directory.\
 Please consider setting the LOCKBOOK_PATH environment variable.";

    env::var("LOCKBOOK_PATH").unwrap_or_else(|_| {
        format!(
            "{}/.lockbook",
            env::var("HOME").unwrap_or_else(|_| env::var("HOMEPATH").expect(ERR_MSG))
        )
    })
}

pub fn get_account(core: &lb::Core) -> Result<Option<lb::Account>, String> {
    match core.get_account() {
        Ok(acct) => Ok(Some(acct)),
        Err(err) => match err {
            lb::Error::UiError(lb::GetAccountError::NoAccount) => Ok(None),
            lb::Error::Unexpected(msg) => Err(msg),
        },
    }
}

pub fn parent_info(
    core: &lb::Core, maybe_id: Option<lb::Uuid>,
) -> Result<(lb::Uuid, String), String> {
    let id = match maybe_id {
        Some(id) => {
            let meta = core.get_file_by_id(id).map_err(|e| format!("{:?}", e))?;

            match meta.file_type {
                lb::FileType::Document => meta.parent,
                lb::FileType::Folder => meta.id,
                lb::FileType::Link { .. } => todo!(),
            }
        }
        None => core.get_root().map_err(|e| format!("{:?}", e))?.id,
    };

    let path = core.get_path_by_id(id).map_err(|e| format!("{:?}", e))?;

    Ok((id, format!("/{}", path)))
}

pub fn save_texture_to_png(
    core: &lb::Core, parent_id: lb::Uuid, texture: gdk::Texture,
) -> Result<lb::File, String> {
    // There's a bit of a chicken and egg situation when it comes to naming a new file based on
    // its id. First, we'll create a new file with a random (temporary) name.
    let mut png_meta = core
        .create_file(&lb::Uuid::new_v4().to_string(), parent_id, lb::FileType::Document)
        .map_err(|err| format!("{:?}", err))?;

    // Then, the file is renamed to its id.
    let png_name = format!("{}.png", png_meta.id);
    core.rename_file(png_meta.id, &png_name)
        .map_err(|err| format!("{:?}", err))?;
    png_meta.name = png_name;

    // Convert the texture to PNG bytes and write them to the newly created lockbook file.
    let png_data = texture.save_to_png_bytes();
    core.write_document(png_meta.id, &png_data)
        .map_err(|err| format!("{:?}", err))?;

    Ok(png_meta)
}

pub fn import_file(
    core: &lb::Core, disk_path: &Path, dest: lb::Uuid, new_file_tx: &glib::Sender<lb::File>,
) -> Result<lb::File, String> {
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
        let siblings = core.get_children(dest).map_err(|e| e.0)?;
        get_non_conflicting_name(&siblings, disk_file_name)
    };

    let file_meta = core
        .create_file(&file_name, dest, file_type)
        .map_err(|e| format!("{:?}", e))?;

    new_file_tx.send(file_meta.clone()).unwrap();

    if file_type == lb::FileType::Document {
        let content = fs::read(&disk_path).map_err(|e| format!("{:?}", e))?;
        core.write_document(file_meta.id, &content)
            .map_err(|e| format!("{:?}", e))?;
    } else {
        let entries = fs::read_dir(disk_path).map_err(|e| format!("{:?}", e))?;
        for entry in entries {
            let child_path = entry.map_err(|e| format!("{:?}", e))?.path();
            import_file(core, &child_path, file_meta.id, new_file_tx)
                .map_err(|e| format!("{:?}", e))?;
        }
    }

    Ok(file_meta)
}

fn get_non_conflicting_name(siblings: &[lb::File], proposed_name: &str) -> String {
    let mut new_name = lb::NameComponents::from(proposed_name);
    loop {
        if !siblings.iter().any(|f| f.name == new_name.to_name()) {
            return new_name.to_name();
        }
        new_name = new_name.generate_next();
    }
}

pub enum SyncProgressReport {
    Update(lb::SyncProgress),
    Done(Result<(), SyncError>),
}

pub enum SyncError {
    Major(String),
    Minor(String),
}

impl From<lb::Error<lb::SyncAllError>> for SyncError {
    fn from(err: lb::Error<lb::SyncAllError>) -> Self {
        match err {
            lb::Error::UiError(err) => Self::Minor(
                match err {
                    lb::SyncAllError::Retry => "Please retry syncing.",
                    lb::SyncAllError::CouldNotReachServer => "Offline.",
                    lb::SyncAllError::ClientUpdateRequired => "Client upgrade required.",
                }
                .to_string(),
            ),
            lb::Error::Unexpected(msg) => Self::Major(msg),
        }
    }
}
