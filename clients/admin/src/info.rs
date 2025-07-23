use std::iter;
use std::time::{SystemTime, UNIX_EPOCH};

use basic_human_duration::ChronoHumanDuration;
use lb::Uuid;
use lb::blocking::Lb;
use lb::model::api::AccountIdentifier;
use lb::model::file_like::FileLike;
use lb::model::lazy::LazyTree;
use lb::model::server_meta::ServerMeta;
use lb::model::tree_like::TreeLike;
use time::Duration;

use crate::Res;

pub fn file(core: &Lb, id: Uuid) -> Res<()> {
    let info = core.admin_file_info(id)?;
    println!("id:\t\t\t{}", info.file.id());
    println!("file_type:\t\t{:?}", info.file.file_type());
    println!("parent:\t\t\t{}", info.file.parent());
    println!(
        "owner:\t\t\t{}",
        core.admin_get_account_info(AccountIdentifier::PublicKey(info.file.owner().0))?
            .username
    );
    println!("explicitly_deleted:\t{}", info.file.explicitly_deleted());
    println!("document_hmac:\t\t{}", info.file.document_hmac().is_some());
    println!("user_access_keys:");
    for k in info.file.user_access_keys() {
        println!(
            "->\tencrypted_by: {}",
            core.admin_get_account_info(AccountIdentifier::PublicKey(k.encrypted_by))?
                .username
        );
        println!(
            "\tencrypted_for: {}",
            core.admin_get_account_info(AccountIdentifier::PublicKey(k.encrypted_for))?
                .username
        );
        println!("\tmode: {:?}", k.mode);
        println!("\tdeleted: {:?}", k.deleted);
    }
    println!();
    let mut tree = iter::once(info.file)
        .chain(info.ancestors)
        .chain(info.descendants)
        .collect::<Vec<_>>()
        .to_lazy();
    pretty_print(&mut tree);
    Ok(())
}

fn pretty_print(tree: &mut LazyTree<Vec<ServerMeta>>) {
    fn print_branch(
        tree: &mut LazyTree<Vec<ServerMeta>>, file_leaf: &ServerMeta, children: &[ServerMeta],
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

fn depth(tree: &mut LazyTree<Vec<ServerMeta>>, root: Uuid) -> usize {
    let mut result = 0;
    for child in tree.children(&root).unwrap() {
        let child_depth = depth(tree, child);
        if child_depth > result {
            result = child_depth;
        }
    }
    result + 1
}
