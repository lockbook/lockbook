use std::cmp::Ordering;
use std::path::Path;

use clap::Parser;

use lb::Core;
use lb::File;
use lb::Uuid;

use crate::CliError;
use crate::ID_PREFIX_LEN;

#[derive(Parser, Debug)]
pub struct ListArgs {
    /// include all children of the given directory, recursively
    #[clap(short, long)]
    recursive: bool,

    /// include more info (such as the file ID)
    #[clap(short, long)]
    long: bool,

    /// display absolute paths instead of just names
    #[clap(long)]
    paths: bool,

    /// only show directories
    #[clap(long)]
    dirs: bool,

    /// only show documents
    #[clap(long)]
    docs: bool,

    /// print full UUIDs instead of truncated ones
    #[clap(long)]
    ids: bool,

    /// file path location whose files will be listed
    #[clap(default_value = "/")]
    directory: String,
}

struct LsConfig {
    my_name: String,
    w_id: usize,
    w_name: usize,
    long: bool,
    paths: bool,
    dirs: bool,
    docs: bool,
}

struct FileNode {
    id: Uuid,
    dirname: String,
    name: String,
    is_dir: bool,
    shared_with_summary: String,
    shared_by: Option<String>,
    children: Vec<FileNode>,
}

fn print_node(node: &FileNode, cfg: &LsConfig) {
    if (!cfg.dirs && !cfg.docs) || (cfg.dirs && node.is_dir) || (cfg.docs && !node.is_dir) {
        println!("{}", node_string(node, cfg));
    }
    for ch in &node.children {
        print_node(ch, cfg);
    }
}

fn node_string(node: &FileNode, cfg: &LsConfig) -> String {
    let mut txt = String::new();
    if cfg.long {
        txt += &format!("{:<w_id$}  ", &node.id.to_string()[..cfg.w_id], w_id = cfg.w_id,);
    }
    let mut np = String::new();
    if cfg.paths {
        np += &node.dirname;
    }
    np += &node.name;
    txt += &format!("{:<w_name$}", np, w_name = cfg.w_name);
    if cfg.long {
        if let Some(shared_by) = &node.shared_by {
            txt += "  @";
            txt += shared_by;
            txt += " ";
        } else {
            txt += "  ";
        }
        if !node.shared_with_summary.is_empty() {
            txt += &format!("-> {}", node.shared_with_summary);
        }
    }
    txt
}

fn get_children(
    core: &Core, files: &[File], parent: Uuid, cfg: &mut LsConfig,
) -> Result<Vec<FileNode>, CliError> {
    let mut children = Vec::new();
    for f in files {
        let is_parent_link = core.get_file_by_id(parent).unwrap().is_link();

        if f.parent == parent || is_parent_link {
            // File name.
            let mut name = f.name.clone();
            if f.is_folder() {
                name += "/";
            }
            // Parent directory.
            let dirname = {
                let path = get_path_by_id(core, f.id)?;
                let mut dn = Path::new(&path)
                    .parent()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                if dn != "/" {
                    dn += "/";
                }
                dn
            };
            // Share info.
            let mut shared_withs = Vec::new();
            let mut shared_by = None;
            for sh in &f.shares {
                if sh.shared_with == cfg.my_name {
                    shared_by = Some(sh.shared_by.clone());
                }
                if sh.shared_with == cfg.my_name {
                    shared_withs.push("me".into());
                } else {
                    shared_withs.push(format!("@{}", sh.shared_with));
                }
            }
            shared_withs.sort_by_key(|s| s.len());
            let shared_with_summary = match shared_withs.len() {
                0 => "".to_string(),
                1 => shared_withs[0].clone(),
                2 => format!("{} and {}", shared_withs[0], shared_withs[1]),
                n => format!("{}, {}, and {} more", shared_withs[0], shared_withs[1], n - 2),
            };
            // Determine column widths.
            {
                let n = if cfg.paths { format!("{}{}", dirname, name).len() } else { name.len() };
                if n > cfg.w_name {
                    cfg.w_name = n;
                }
            }
            let child = FileNode {
                id: f.id,
                dirname,
                name,
                is_dir: f.is_folder(),
                shared_with_summary,
                shared_by,
                children: if is_parent_link {
                    vec![]
                } else {
                    get_children(core, files, f.id, cfg)?
                },
            };
            children.push(child);
        }
    }
    children.sort_by(|a, b| {
        if a.is_dir && !b.is_dir {
            return Ordering::Less;
        }
        if !a.is_dir && b.is_dir {
            return Ordering::Greater;
        }
        a.name.cmp(&b.name)
    });
    Ok(children)
}

pub fn list(core: &Core, args: ListArgs) -> Result<(), CliError> {
    let id = core
        .get_by_path(&args.directory)
        .map_err(|err| (err, args.directory.as_str()))?
        .id;

    let mut files = if args.recursive {
        core.get_and_get_children_recursively(id)
            .map_err(|err| (err, id))?
    } else {
        core.get_children(id)?
    };
    // Discard root if present. This guarantees every file to have a `dirname` and `name`.
    if let Some(pos) = files.iter().position(|f| f.id == f.parent) {
        let _ = files.swap_remove(pos);
    }

    let w_id = if args.ids { Uuid::nil().to_string().len() } else { ID_PREFIX_LEN };

    let mut cfg = LsConfig {
        my_name: core.get_account()?.username,
        w_id,
        w_name: 0,
        long: args.long,
        paths: args.paths,
        dirs: args.dirs,
        docs: args.docs,
    };
    if args.ids {
        cfg.long = true;
    }

    for ch in get_children(core, &files, id, &mut cfg)? {
        print_node(&ch, &cfg);
    }
    Ok(())
}

fn get_path_by_id(core: &Core, id: Uuid) -> Result<String, CliError> {
    core.get_path_by_id(id)
        .map_err(|err| CliError::new(format!("getting path for id '{}': {:?}", id, err)))
}
