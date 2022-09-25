extern crate core;

mod account;
mod disappear;
mod error;
mod validate;

use lockbook_core::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, iter};

use structopt::StructOpt;

use crate::error::Error;
use lockbook_core::{
    ChronoHumanDuration, Config, Core, FileLike, LazyTree, ServerFile, Stagable, TreeLike, Uuid,
};

#[derive(Debug, PartialEq, Eq, StructOpt)]
pub enum Admin {
    /// Disappear a user
    ///
    /// Frees up their username
    DisappearAccount { username: String },

    /// Disappear a file
    ///
    /// When you delete a file you flip that file's is_deleted flag to false. In a disaster recovery
    /// scenario, you may want to *disappear* a file so that it never existed. This is useful in a
    /// scenario where your server let in an invalid file.
    DisappearFile { id: Uuid },

    /// Validates file trees of all users on the server and prints any failures
    ValidateAccount { username: String },

    /// Performs server-wide integrity checks
    ValidateServer,

    /// List all users
    ListUsers {
        #[structopt(short, long)]
        premium: bool,

        #[structopt(short, long)]
        google_play_premium: bool,

        #[structopt(short, long)]
        stripe_premium: bool,
    },

    /// Get a user's info. This includes their username, public key, and payment platform.
    AccountInfo {
        #[structopt(short, long)]
        username: Option<String>,

        // A base 64 encoded and compressed public key
        #[structopt(short, long)]
        public_key: Option<String>,
    },

    /// Prints information about a file as it appears on the server
    FileInfo { id: Uuid },
}

type Res<T> = Result<T, Error>;

pub fn main() {
    let writeable_path = match (env::var("LOCKBOOK_PATH"), env::var("HOME"), env::var("HOMEPATH")) {
        (Ok(s), _, _) => s,
        (Err(_), Ok(s), _) => format!("{}/.lockbook/cli", s),
        (Err(_), Err(_), Ok(s)) => format!("{}/.lockbook/cli", s),
        _ => panic!("no lockbook location"),
    };

    let core = Core::init(&Config { writeable_path, logs: true, colored_logs: true }).unwrap();

    let result = match Admin::from_args() {
        Admin::DisappearAccount { username } => disappear::account(&core, username),
        Admin::ListUsers { premium, google_play_premium, stripe_premium } => {
            account::list(&core, premium, google_play_premium, stripe_premium)
        }
        Admin::AccountInfo { username, public_key } => account::info(&core, username, public_key),
        Admin::DisappearFile { id } => disappear::file(&core, id),
        Admin::ValidateAccount { username } => validate::account(&core, username),
        Admin::ValidateServer => validate::server(&core),
        Admin::FileInfo { id } => file_info(&core, id),
    };

    if result.is_err() {
        panic!("unsuccessful completion")
    }
}

fn file_info(core: &Core, id: Uuid) -> Res<()> {
    let info = core.admin_file_info(id)?;
    let mut tree = iter::once(info.file)
        .chain(info.ancestors.into_iter())
        .chain(info.descendants.into_iter())
        .collect::<Vec<_>>()
        .to_lazy();
    pretty_print(&mut tree);
    Ok(())
}

fn pretty_print(tree: &mut LazyTree<Vec<ServerFile>>) {
    fn print_branch(
        tree: &mut LazyTree<Vec<ServerFile>>, file_leaf: &ServerFile, children: &[ServerFile],
        branch: &str, crotch: &str, twig: &str, depth: usize,
    ) -> String {
        let last_modified = {
            Duration::milliseconds(
                (SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    - file_leaf.version as u128) as i64,
            )
            .format_human()
            .to_string()
        };
        let mut sub_tree = format!(
            "{}{}{}{}{}|{}|{}|{}|{}\n",
            branch,
            twig,
            file_leaf.file.id(),
            " ".repeat(depth * 4 - (branch.chars().count() + twig.chars().count())),
            if file_leaf.is_document() {
                "doc "
            } else if file_leaf.is_folder() {
                "dir "
            } else {
                "link"
            },
            if file_leaf.document_hmac().is_some() { "some" } else { "none" },
            if file_leaf.explicitly_deleted() { "yes" } else { "no " },
            if file_leaf.is_shared() { "yes  " } else { "no   " },
            last_modified
        );
        let mut next_branch = branch.to_string();
        next_branch.push_str(crotch);

        let num_children = children.len();

        for (count, child) in children.iter().enumerate() {
            let next_children_ids = tree.children(child.id()).unwrap();
            let next_children = next_children_ids
                .into_iter()
                .filter_map(|id| tree.maybe_find(&id))
                .cloned()
                .collect::<Vec<_>>();

            let last_child = count == num_children - 1;

            let next_crotch = if next_children.is_empty() {
                ""
            } else if last_child {
                "    "
            } else {
                "│   "
            };

            let next_twig = if last_child { "└── " } else { "├── " };

            sub_tree.push_str(&print_branch(
                tree,
                child,
                &next_children,
                &next_branch,
                next_crotch,
                next_twig,
                depth,
            ));
        }

        sub_tree
    }

    let mut maybe_root = None;
    for meta in tree.all_files().unwrap() {
        if meta.is_root() {
            maybe_root = Some(meta.clone());
            break;
        }
    }
    if let Some(root) = maybe_root {
        let children_ids = tree.children(root.id()).unwrap();
        let children = children_ids
            .into_iter()
            .filter_map(|id| tree.maybe_find(&id))
            .cloned()
            .collect::<Vec<_>>();
        let depth = depth(tree, *root.id());
        println!("TREE{} TYPE|HMAC|DEL|SHARE|MODIFIED", " ".repeat(31 + depth * 4));
        println!("{}", print_branch(tree, &root, &children, "", "", "", depth));
    } else {
        panic!("failed to find root");
    }
}

fn depth(tree: &mut LazyTree<Vec<ServerFile>>, root: Uuid) -> usize {
    let mut result = 0;
    for child in tree.children(&root).unwrap() {
        let child_depth = depth(tree, child);
        if child_depth > result {
            result = child_depth;
        }
    }
    result + 1
}
