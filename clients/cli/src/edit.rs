use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use hotwatch::{Event, Hotwatch};

use lb::{Core, Uuid};

use crate::resolve_target_to_file;
use crate::CliError;

pub fn edit(core: &Core, target: &str) -> Result<(), CliError> {
    let f = resolve_target_to_file(core, target)?;

    let file_content = core.read_document(f.id)?;

    let mut temp_file_path = create_tmp_dir()?;
    temp_file_path.push(f.name);

    let mut file_handle = fs::File::create(&temp_file_path)
        .map_err(|err| CliError(format!("couldn't open temporary file for writing: {:#?}", err)))?;
    file_handle.write_all(&file_content)?;
    file_handle.sync_all()?;

    let maybe_watcher = set_up_auto_save(core, f.id, &temp_file_path);
    let edit_was_successful = edit_file_with_editor(&temp_file_path);

    if let Some(mut watcher) = maybe_watcher {
        watcher
            .unwatch(&temp_file_path)
            .unwrap_or_else(|err| eprintln!("file watcher failed to unwatch: {:#?}", err))
    }

    if edit_was_successful {
        match save_temp_file_contents(core, f.id, &temp_file_path) {
            Ok(_) => println!("Document encrypted and saved. Cleaning up temporary file."),
            Err(err) => eprintln!("{}", err),
        }
    } else {
        eprintln!("Your editor indicated a problem, aborting and cleaning up");
    }

    fs::remove_file(&temp_file_path)?;
    Ok(())
}

fn create_tmp_dir() -> Result<PathBuf, CliError> {
    let mut dir = std::env::temp_dir();
    dir.push(Uuid::new_v4().to_string());
    fs::create_dir(&dir)
        .map_err(|err| CliError(format!("couldn't open temporary file for writing: {:#?}", err)))?;
    Ok(dir)
}

// In ascending order of superiority
#[derive(Debug)]
enum Editor {
    Vim,
    Emacs,
    Nvim,
    Nano,
    Sublime,
    Code,
}

fn get_editor() -> Editor {
    let default_editor = if cfg!(target_os = "windows") { Editor::Code } else { Editor::Vim };
    match std::env::var("LOCKBOOK_EDITOR") {
        Ok(editor) => match editor.to_lowercase().as_str() {
            "vim" => Editor::Vim,
            "emacs" => Editor::Emacs,
            "nvim" => Editor::Nvim,
            "nano" => Editor::Nano,
            "subl" | "sublime" => Editor::Sublime,
            "code" => Editor::Code,
            _ => {
                eprintln!(
                    "{} is not yet supported, make a github issue! Falling back to {:?}.",
                    editor, default_editor
                );
                default_editor
            }
        },
        Err(_) => {
            eprintln!("LOCKBOOK_EDITOR not set, assuming {:?}", default_editor);
            default_editor
        }
    }
}

#[cfg(target_os = "windows")]
fn edit_file_with_editor<S: AsRef<Path>>(path: S) -> bool {
    let path_str = path.as_ref().display();

    let command = match get_editor() {
        Editor::Vim | Editor::Nvim | Editor::Emacs | Editor::Nano => {
            eprintln!("Terminal editors are not supported on windows! Set LOCKBOOK_EDITOR to a visual editor.");
            return false;
        }
        Editor::Sublime => format!("subl --wait {}", path_str),
        Editor::Code => format!("code --wait {}", path_str),
    };

    std::process::Command::new("cmd")
        .arg("/C")
        .arg(command)
        .spawn()
        .expect("Error: Failed to run editor")
        .wait()
        .unwrap()
        .success()
}

#[cfg(not(target_os = "windows"))]
fn edit_file_with_editor<S: AsRef<Path>>(path: S) -> bool {
    let path_str = path.as_ref().display();

    let command = match get_editor() {
        Editor::Vim => format!("</dev/tty vim {}", path_str),
        Editor::Nvim => format!("</dev/tty nvim {}", path_str),
        Editor::Emacs => format!("</dev/tty emacs {}", path_str),
        Editor::Nano => format!("</dev/tty nano {}", path_str),
        Editor::Sublime => format!("subl --wait {}", path_str),
        Editor::Code => format!("code --wait {}", path_str),
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

fn set_up_auto_save<P: AsRef<Path>>(core: &Core, id: Uuid, path: P) -> Option<Hotwatch> {
    match Hotwatch::new_with_custom_delay(core::time::Duration::from_secs(5)) {
        Ok(mut watcher) => {
            let core = core.clone();
            let path = PathBuf::from(path.as_ref());

            watcher
                .watch(path.clone(), move |event: Event| match event {
                    Event::NoticeWrite(_) | Event::Write(_) | Event::Create(_) => {
                        if let Err(err) = save_temp_file_contents(&core, id, &path) {
                            eprintln!("{}", err);
                        }
                    }
                    _ => {}
                })
                .unwrap_or_else(|err| println!("file watcher failed to watch: {:#?}", err));

            Some(watcher)
        }
        Err(err) => {
            println!("file watcher failed to initialize: {:#?}", err);
            None
        }
    }
}

fn save_temp_file_contents<P: AsRef<Path>>(core: &Core, id: Uuid, path: P) -> Result<(), CliError> {
    let secret = fs::read_to_string(&path)
        .map_err(|err| {
            CliError(format!(
                "could not read from temporary file, not deleting {}, err: {:#?}",
                path.as_ref().display(),
                err
            ))
        })?
        .into_bytes();

    core.write_document(id, &secret)?;
    Ok(())
}
