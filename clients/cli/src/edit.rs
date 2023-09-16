use std::{
    convert::Infallible,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
};

use cli_rs::{
    cli_error::{CliError, CliResult},
    flag::Flag,
};
use hotwatch::{Event, EventKind, Hotwatch};
use lb::{Core, Uuid};

use crate::{ensure_account_and_root, input::FileInput};

pub fn edit(core: &Core, editor: Editor, target: FileInput) -> CliResult<()> {
    ensure_account_and_root(core)?;

    let f = target.find(core)?;

    let file_content = core.read_document(f.id)?;

    let mut temp_file_path = create_tmp_dir()?;
    temp_file_path.push(f.name);

    let mut file_handle = fs::File::create(&temp_file_path).map_err(|err| {
        CliError::from(format!("couldn't open temporary file for writing: {:#?}", err))
    })?;
    file_handle.write_all(&file_content)?;
    file_handle.sync_all()?;

    let maybe_watcher = set_up_auto_save(core, f.id, &temp_file_path);
    let edit_was_successful = edit_file_with_editor(editor, &temp_file_path);

    if let Some(mut watcher) = maybe_watcher {
        watcher
            .unwatch(&temp_file_path)
            .unwrap_or_else(|err| eprintln!("file watcher failed to unwatch: {:#?}", err))
    }

    if edit_was_successful {
        match save_temp_file_contents(core, f.id, &temp_file_path) {
            Ok(_) => println!("Document encrypted and saved. Cleaning up temporary file."),
            Err(err) => eprintln!("{:?}", err),
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
    fs::create_dir(&dir).map_err(|err| {
        CliError::from(format!("couldn't open temporary file for writing: {:#?}", err))
    })?;
    Ok(dir)
}

// In ascending order of superiority
#[derive(Debug, Clone, Copy)]
pub enum Editor {
    Vim,
    Nvim,
    Emacs,
    Nano,
    Sublime,
    Code,
}

impl Default for Editor {
    fn default() -> Self {
        let default = if cfg!(target_os = "windows") { Editor::Code } else { Editor::Vim };

        env::var("LOCKBOOK_EDITOR")
            .map(|s| s.parse().unwrap())
            .unwrap_or_else(|_| {
                eprintln!("LOCKBOOK_EDITOR not set, assuming {:?}", default);
                default
            })
    }
}

pub fn editor_flag() -> Flag<'static, Editor> {
    Flag::new("editor")
        .description("optional editor flag, if not present falls back to LOCKBOOK_EDITOR, if not present falls back to a platform default")
        .completor(|prompt| {
            Ok(["vim", "nvim", "emacs", "nano", "sublime", "code"]
                .into_iter()
                .filter(|entry| entry.starts_with(prompt))
                .map(|s| s.to_string())
                .collect())
    })
}

impl FromStr for Editor {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let editor = match s.to_lowercase().as_str() {
            "vim" => Editor::Vim,
            "nvim" => Editor::Nvim,
            "emacs" => Editor::Emacs,
            "nano" => Editor::Nano,
            "subl" | "sublime" => Editor::Sublime,
            "code" => Editor::Code,
            unsupported => {
                let default = Editor::default();
                eprintln!(
                    "{} is not yet supported, make a github issue! Falling back to {:?}.",
                    unsupported, default
                );
                default
            }
        };

        Ok(editor)
    }
}

#[cfg(target_os = "windows")]
fn edit_file_with_editor<S: AsRef<Path>>(editor: Editor, path: S) -> bool {
    let path_str = path.as_ref().display();

    let command = match editor {
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
fn edit_file_with_editor<S: AsRef<Path>>(editor: Editor, path: S) -> bool {
    let path_str = path.as_ref().display();

    let command = match editor {
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
                .watch(path.clone(), move |event: Event| {
                    if let EventKind::Modify(_) = event.kind {
                        if let Err(err) = save_temp_file_contents(&core, id, &path) {
                            eprintln!("{:?}", err);
                        }
                    }
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
            CliError::from(format!(
                "could not read from temporary file, not deleting {}, err: {:#?}",
                path.as_ref().display(),
                err
            ))
        })?
        .into_bytes();

    core.write_document(id, &secret)?;
    Ok(())
}
