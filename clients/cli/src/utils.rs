use std::{env, fs};

use hotwatch::{Event, Hotwatch};
use uuid::Uuid;

use lockbook_core::model::errors::WriteToDocumentError;
use lockbook_core::pure_functions::drawing::SupportedImageFormats;
use lockbook_core::Core;
use lockbook_core::Error as LbError;

use crate::error::CliError;

// In ascending order of superiority
pub enum SupportedEditors {
    Vim,
    Emacs,
    Nano,
    Sublime,
    Code,
}

pub fn get_editor() -> SupportedEditors {
    use SupportedEditors::*;
    match env::var("LOCKBOOK_EDITOR") {
        Ok(editor) => match editor.to_lowercase().as_str() {
            "vim" => Vim,
            "emacs" => Emacs,
            "nano" => Nano,
            "subl" | "sublime" => Sublime,
            "code" => Code,
            _ => {
                eprintln!(
                    "{} is not yet supported, make a github issue! Falling back to vim.",
                    editor
                );
                Vim
            }
        },
        Err(_) => {
            eprintln!("LOCKBOOK_EDITOR not set, assuming vim");
            Vim
        }
    }
}

pub fn get_directory_location() -> Result<String, CliError> {
    let result = format!("{}/{}", env::temp_dir().to_str().unwrap_or("/tmp"), Uuid::new_v4());
    fs::create_dir(&result).map_err(|err| {
        CliError::unexpected(format!("couldn't open temporary file for writing: {:#?}", err))
    })?;
    Ok(result)
}

pub fn get_image_format(image_format: &str) -> SupportedImageFormats {
    use SupportedImageFormats::*;

    match image_format.to_lowercase().as_str() {
        "png" => Png,
        "jpeg" | "jpg" => Jpeg,
        "bmp" => Bmp,
        "tga" => Tga,
        "pnm" => Pnm,
        "farbfeld" => Farbfeld,
        _ => {
            eprintln!(
                "{} is not yet supported, make a github issue! Falling back to png.",
                image_format
            );
            Png
        }
    }
}

pub fn edit_file_with_editor(file_location: &str) -> bool {
    use SupportedEditors::*;

    if cfg!(target_os = "windows") {
        let command = match get_editor() {
            Vim | Emacs | Nano => {
                eprintln!("Terminal editors are not supported on windows! Set LOCKBOOK_EDITOR to a visual editor.");
                return false;
            }
            Sublime => format!("subl --wait {}", file_location),
            Code => format!("code --wait {}", file_location),
        };

        std::process::Command::new("cmd")
            .arg("/C")
            .arg(command)
            .spawn()
            .expect("Error: Failed to run editor")
            .wait()
            .unwrap()
            .success()
    } else {
        let command = match get_editor() {
            Vim => format!("</dev/tty vim {}", file_location),
            Emacs => format!("</dev/tty emacs {}", file_location),
            Nano => format!("</dev/tty nano {}", file_location),
            Sublime => format!("subl --wait {}", file_location),
            Code => format!("code --wait {}", file_location),
        };

        std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .spawn()
            .expect("Error: Failed to run editor")
            .wait()
            .unwrap()
            .success()
    }
}

pub fn print_last_successful_sync(core: &Core) -> Result<(), CliError> {
    if atty::is(atty::Stream::Stdout) {
        let last_updated = core.get_last_synced_human_string().map_err(|err| {
            CliError::unexpected(format!("attempting to retrieve usage: {:#?}", err))
        })?;

        println!("Last successful sync: {}", last_updated);
    }
    Ok(())
}

pub fn set_up_auto_save(core: &Core, id: Uuid, location: String) -> Option<Hotwatch> {
    let core = core.clone();
    match Hotwatch::new_with_custom_delay(core::time::Duration::from_secs(5)) {
        Ok(mut watcher) => {
            watcher
                .watch(location.clone(), move |event: Event| match event {
                    Event::NoticeWrite(_) | Event::Write(_) | Event::Create(_) => {
                        if let Err(err) = save_temp_file_contents(&core, id, &location) {
                            err.print();
                        }
                    }
                    _ => {}
                })
                .unwrap_or_else(|err| {
                    println!("file watcher failed to watch: {:#?}", err);
                });

            Some(watcher)
        }
        Err(err) => {
            println!("file watcher failed to initialize: {:#?}", err);
            None
        }
    }
}

pub fn stop_auto_save(mut watcher: Hotwatch, file_location: String) {
    watcher
        .unwatch(file_location)
        .unwrap_or_else(|err| eprintln!("file watcher failed to unwatch: {:#?}", err))
}

pub fn save_temp_file_contents(core: &Core, id: Uuid, location: &str) -> Result<(), CliError> {
    let secret = fs::read_to_string(&location)
        .map_err(|err| {
            CliError::unexpected(format!(
                "could not read from temporary file, not deleting {}, err: {:#?}",
                location, err
            ))
        })?
        .into_bytes();

    core.write_document(id, &secret).map_err(|err| match err {
        LbError::UiError(err) => match err {
            WriteToDocumentError::NoAccount => CliError::no_account(),
            WriteToDocumentError::FileDoesNotExist => CliError::unexpected("file doesn't exist"),
            WriteToDocumentError::FolderTreatedAsDocument => {
                CliError::unexpected("can't write to folder")
            }
        },
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })
}
