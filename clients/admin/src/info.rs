use lockbook_core::Duration;
use std::iter;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Res;
use lockbook_core::{
    ChronoHumanDuration, Core, FileLike, LazyTree, ServerFile, Stagable, TreeLike, Uuid,
};

pub fn file(core: &Core, id: Uuid) -> Res<()> {
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
