//! `--try <fixture> --try-out <out>`: the app on a throwaway account whose core and server live in
//! this process, holding a QA fixture's files and open to its `qa.md`. On quit, `qa.md`'s text is
//! written to `<out>`.

use std::path::PathBuf;
use std::{fs, io};

use lb::Uuid;
use lb::model::core_config::{ClientType, Config};
use lb::model::errors::{LbErrKind, LbResult};
use lockbook_server_lib::in_process::InMemoryServer;

/// The core a try runs on.
pub type Lb = lb::blocking::Lb<InMemoryServer>;

/// The document a fixture opens to and that a try writes out.
pub const QA: &str = "qa.md";

pub struct Try {
    /// The fixture's folder: `qa.md` and the files around it.
    pub fixture: PathBuf,
    /// Where `qa.md`'s text goes on quit.
    pub out: PathBuf,
    /// The data directory for the run, in place of the user's.
    pub home: PathBuf,
}

impl Try {
    /// The try `args` ask for, if any.
    pub fn from_args(args: &[String]) -> Option<Result<Self, String>> {
        let value = |flag: &str| {
            args.iter().position(|a| a == flag).map(|i| {
                args.get(i + 1)
                    .cloned()
                    .ok_or(format!("{flag} takes a path"))
            })
        };
        let fixture = value("--try")?;
        Some((|| {
            let fixture = PathBuf::from(fixture?);
            let out = PathBuf::from(value("--try-out").ok_or("--try needs --try-out <path>")??);
            let home = std::env::temp_dir().join(format!("lockbook-try-{}", Uuid::new_v4()));
            Ok(Self { fixture, out, home })
        })())
    }

    pub fn config(&self) -> Config {
        Config {
            client_name: "egui".into(),
            writeable_path: self.home.to_string_lossy().into_owned(),
            background_work: true,
            logs: false,
            stdout_logs: true,
            colored_logs: true,
            client_type: ClientType::Ui,
            ..Default::default()
        }
    }

    /// A new account on a new in-memory server, holding the fixture's files.
    pub fn world(&self) -> LbResult<Lb> {
        let lb = Lb::in_memory(self.config(), InMemoryServer::in_memory())?;
        lb.create_account("tester", "in-process", false)?;
        let root = lb.get_root()?;
        let sources = fs::read_dir(&self.fixture)
            .and_then(|entries| {
                entries
                    .map(|e| e.map(|e| e.path()))
                    .collect::<io::Result<Vec<_>>>()
            })
            .map_err(|e| {
                LbErrKind::Unexpected(format!("reading {}: {e}", self.fixture.display()))
            })?;
        lb.import_files(&sources, root.id, &|_| {})?;
        Ok(lb)
    }

    /// The link that opens the fixture's `qa.md`.
    pub fn qa_link(lb: &Lb) -> LbResult<String> {
        Ok(format!("lb://{}", qa(lb)?))
    }

    /// Writes `qa.md`'s text to [Self::out] and removes the run's data directory.
    pub fn finish(&self, lb: &Lb) -> io::Result<()> {
        let text = qa(lb)
            .and_then(|id| lb.read_document(id, false))
            .map_err(|e| io::Error::other(format!("reading {QA}: {e:?}")))?;
        fs::write(&self.out, text)?;
        let _ = fs::remove_dir_all(&self.home);
        Ok(())
    }
}

fn qa(lb: &Lb) -> LbResult<Uuid> {
    Ok(lb.get_by_path(&format!("/{QA}"))?.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn a_try_keeps_out_of_home_and_writes_out_its_edits() {
        let scratch = std::env::temp_dir().join(format!("lockbook-try-test-{}", Uuid::new_v4()));
        let home = scratch.join("home");
        fs::create_dir_all(&home).unwrap();
        // SAFETY: the only test in this crate that reads or writes the environment.
        unsafe { std::env::set_var("HOME", &home) };

        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../qa/try-mode");
        let args = ["--try", fixture.to_str().unwrap(), "--try-out"].map(String::from);
        let out = scratch.join("out.md");
        let args = [&args[..], &[out.to_string_lossy().into_owned()]].concat();
        let try_mode = Try::from_args(&args).unwrap().unwrap();

        let lb = try_mode.world().unwrap();
        assert!(lb.get_by_path("/other.md").is_ok(), "the fixture's other files are in the world");
        let link = Try::qa_link(&lb).unwrap();
        let qa = Uuid::parse_str(link.strip_prefix("lb://").unwrap()).unwrap();
        let mut text = lb.read_document(qa, false).unwrap();
        text.extend_from_slice(b"- [x] ticked\n");
        lb.write_document(qa, &text).unwrap();
        try_mode.finish(&lb).unwrap();

        assert_eq!(fs::read(&out).unwrap(), text);
        assert_eq!(fs::read_dir(&home).unwrap().count(), 0, "nothing was written to HOME");
        assert!(!try_mode.home.exists(), "the run's data directory is gone");
        fs::remove_dir_all(&scratch).unwrap();
    }
}
