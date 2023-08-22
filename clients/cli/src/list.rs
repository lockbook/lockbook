use std::{cmp::Ordering, path::Path};

use cli_rs::cli_error::CliResult;
use lb::{Core, File, Uuid};

use crate::{ensure_account_and_root, input::FileInput};

const ID_PREFIX_LEN: usize = 8;

pub fn list(
    core: &Core, long: bool, recursive: bool, mut paths: bool, target: FileInput,
) -> CliResult<()> {
    ensure_account_and_root(core)?;

    let id = target.find(core)?.id;

    let mut files =
        if recursive { core.get_and_get_children_recursively(id)? } else { core.get_children(id)? };

    // Discard root if present. This guarantees every file to have a `dirname` and `name`.
    if let Some(pos) = files.iter().position(|f| f.id == f.parent) {
        let _ = files.swap_remove(pos);
    }

    if recursive {
        paths = true
    };

    let mut cfg = LsConfig {
        my_name: core.get_account()?.username,
        w_id: ID_PREFIX_LEN,
        w_name: 0,
        long,
        paths,
    };

    for ch in get_children(core, &files, id, &mut cfg)? {
        print_node(&ch, &cfg);
    }
    Ok(())
}

struct LsConfig {
    my_name: String,
    w_id: usize,
    w_name: usize,
    long: bool,
    paths: bool,
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
    println!("{}", node_string(node, cfg));

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
) -> CliResult<Vec<FileNode>> {
    let mut children = Vec::new();
    for f in files {
        if f.parent == parent {
            // File name.
            let mut name = f.name.clone();
            if f.is_folder() {
                name += "/";
            }
            // Parent directory.
            let dirname = {
                let path = core.get_path_by_id(f.id)?;
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
                children: get_children(core, files, f.id, cfg)?,
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
