use std::cmp::Ordering;
use std::path::Path;

use cli_rs::cli_error::CliResult;
use lb_rs::model::file::File;
use lb_rs::{Lb, Uuid};

use crate::input::FileInput;
use crate::{core, ensure_account_and_root};

const ID_PREFIX_LEN: usize = 8;

#[tokio::main]
pub async fn list(
    long: bool, recursive: bool, mut paths: bool, target: FileInput,
) -> CliResult<()> {
    let lb = &core().await?;
    ensure_account_and_root(lb).await?;

    let id = target.find(lb).await?.id;

    let mut files = if recursive {
        lb.get_and_get_children_recursively(&id).await?
    } else {
        lb.get_children(&id).await?
    };

    // Discard root if present. This guarantees every file to have a `dirname` and `name`.
    if let Some(pos) = files.iter().position(|f| f.id == f.parent) {
        let _ = files.swap_remove(pos);
    }

    if recursive {
        paths = true
    };

    let mut cfg = LsConfig {
        my_name: lb.get_account()?.username.clone(),
        w_id: ID_PREFIX_LEN,
        w_name: 0,
        long,
        paths,
    };

    for ch in get_children(lb, &files, id, &mut cfg).await? {
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

async fn get_children(
    lb: &Lb, files: &[File], parent: Uuid, cfg: &mut LsConfig,
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
                let path = lb.get_path_by_id(f.id).await?;
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
                let n = if cfg.paths { format!("{dirname}{name}").len() } else { name.len() };
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
                children: Box::pin(get_children(lb, files, f.id, cfg)).await?,
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
