use std::convert::Infallible;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::{env, fs};

use cli_rs::cli_error::{CliError, CliResult};
use cli_rs::flag::Flag;
use hotwatch::{Event, EventKind, Hotwatch};
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::model::text::buffer::Buffer as TextBuffer;
use lb_rs::{Lb, Uuid};
use tokio::runtime::Handle;
use tokio::sync::Mutex;

use crate::input::find_file;
use crate::{core, ensure_account_and_root};

struct Base {
    hmac: Option<DocumentHmac>,
    text: String,
}

#[tokio::main]
pub async fn edit(editor: Editor, target: String) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let f = find_file(lb, &target).await?;
    let (hmac, file_content) = lb.read_document_with_hmac(f.id, true).await?;
    let text = String::from_utf8_lossy(&file_content).into_owned();

    let mut temp_file_path = create_tmp_dir()?;
    temp_file_path.push(f.name);

    let mut file_handle = fs::File::create(&temp_file_path).map_err(|err| {
        CliError::from(format!("couldn't open temporary file for writing: {err:#?}"))
    })?;
    file_handle.write_all(&file_content)?;
    file_handle.sync_all()?;

    let base = Arc::new(Mutex::new(Base { hmac, text }));
    let maybe_watcher = set_up_auto_save(lb, f.id, &temp_file_path, base.clone());
    let edit_was_successful = edit_file_with_editor(editor, &temp_file_path);

    if let Some(mut watcher) = maybe_watcher {
        watcher
            .unwatch(&temp_file_path)
            .unwrap_or_else(|err| eprintln!("file watcher failed to unwatch: {err:#?}"))
    }

    if edit_was_successful {
        match save(lb.clone(), f.id, &temp_file_path, base).await {
            Ok(_) => println!("Document encrypted and saved. Cleaning up temporary file."),
            Err(err) => eprintln!("{err:?}"),
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
        CliError::from(format!("couldn't open temporary file for writing: {err:#?}"))
    })?;
    Ok(dir)
}

// In ascending order of superiority
#[derive(Debug, Clone)]
pub enum Editor {
    Vim,
    Nvim,
    Emacs,
    Helix,
    Nano,
    Sublime,
    Code,
    Custom(String),
}

impl Default for Editor {
    fn default() -> Self {
        let default = if cfg!(target_os = "windows") { Editor::Code } else { Editor::Vim };

        env::var("LOCKBOOK_EDITOR")
            .map(Editor::Custom)
            .or(Self::from_sys_env_var())
            .unwrap_or_else(|_| {
                eprintln!("LOCKBOOK_EDITOR, VISUAL or EDITOR not set, assuming {default:?}");
                default
            })
    }
}

impl Editor {
    fn from_sys_env_var() -> CliResult<Self> {
        let editor = env::var("EDITOR")
            .or(env::var("VISUAL"))
            .map_err(|_| "no EDITOR or VISUAL")?;

        let editor = editor.split('/').next_back().unwrap();

        Ok(editor.parse().map_err(|_| "no EDITOR or VISUAL")?)
    }
}

pub fn editor_flag() -> Flag<'static, Editor> {
    Flag::new("editor")
        .description("optional editor flag, if not present falls back to LOCKBOOK_EDITOR, if not present falls back to a platform default")
        .completor(|prompt| {
            Ok(["vim", "nvim", "emacs", "helix", "nano", "sublime", "code"]
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
            "hx" | "helix" => Editor::Helix,
            "nano" => Editor::Nano,
            "subl" | "sublime" => Editor::Sublime,
            "code" => Editor::Code,
            unsupported => {
                let default = Editor::default();
                eprintln!(
                    "{unsupported} is not yet supported, make a github issue! Falling back to {default:?}."
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
        Editor::Vim | Editor::Nvim | Editor::Emacs | Editor::Nano | Editor::Helix => {
            eprintln!(
                "Terminal editors are not supported on windows! Set LOCKBOOK_EDITOR to a visual editor."
            );
            return false;
        }
        Editor::Sublime => format!("subl --wait {path_str}"),
        Editor::Code => format!("code --wait {path_str}"),
        Editor::Custom(s) => format!("{s} {path_str}"),
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
        Editor::Vim => format!("</dev/tty vim '{path_str}'"),
        Editor::Nvim => format!("</dev/tty nvim '{path_str}'"),
        Editor::Emacs => format!("</dev/tty emacs '{path_str}'"),
        Editor::Helix => format!("</dev/tty hx '{path_str}'"),
        Editor::Nano => format!("</dev/tty nano '{path_str}'"),
        Editor::Sublime => format!("subl --wait '{path_str}'"),
        Editor::Code => format!("code --wait '{path_str}'"),
        Editor::Custom(s) => format!("{s} '{path_str}'"),
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

fn set_up_auto_save(core: &Lb, id: Uuid, path: &Path, base: Arc<Mutex<Base>>) -> Option<Hotwatch> {
    match Hotwatch::new_with_custom_delay(core::time::Duration::from_secs(5)) {
        Ok(mut watcher) => {
            let core = core.clone();
            let path = PathBuf::from(path);
            let handle = Handle::current();

            watcher
                .watch(path.clone(), move |event: Event| {
                    if let EventKind::Modify(_) = event.kind {
                        let core = core.clone();
                        let path = path.clone();
                        let base = base.clone();
                        handle.spawn(async move {
                            if let Err(err) = save(core, id, &path, base).await {
                                eprintln!("autosave failed: {err:?}");
                            }
                        });
                    }
                })
                .unwrap_or_else(|err| println!("file watcher failed to watch: {err:#?}"));

            Some(watcher)
        }
        Err(err) => {
            println!("file watcher failed to initialize: {err:#?}");
            None
        }
    }
}

/// CAS write of the temp file. On concurrent change, 3-way merge then retry (chat `write_back` pattern).
async fn save(lb: Lb, id: Uuid, path: &Path, base: Arc<Mutex<Base>>) -> Result<(), CliError> {
    let mut base = base.lock().await;
    let ours = fs::read_to_string(path).map_err(|err| {
        CliError::from(format!(
            "could not read from temporary file, not deleting {}, err: {err:#?}",
            path.display()
        ))
    })?;
    if ours == base.text {
        return Ok(());
    }

    loop {
        let (disk_hmac, disk_bytes) = lb.read_document_with_hmac(id, false).await?;
        let clean = disk_hmac == base.hmac;
        let to_write = if clean {
            ours.clone()
        } else {
            let theirs = String::from_utf8_lossy(&disk_bytes);
            TextBuffer::from(base.text.as_str()).merge(ours.clone(), theirs.into_owned())
        };

        match lb
            .safe_write(id, disk_hmac, to_write.clone().into_bytes(), None)
            .await
        {
            Ok(new_hmac) => {
                if !clean {
                    eprintln!("Merged concurrent changes into the document.");
                    let _ = fs::write(path, &to_write);
                }
                base.hmac = Some(new_hmac);
                base.text = to_write;
                return Ok(());
            }
            Err(e) if matches!(e.kind, LbErrKind::ReReadRequired) => continue,
            Err(e) => return Err(CliError::from(e.to_string())),
        }
    }
}
