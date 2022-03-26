use crate::error::CliResult;
use crate::utils::{account, config};
use lockbook_core::{get_children, get_root};
use lockbook_models::file_metadata::DecryptedFileMetadata;
use lockbook_core::model::state::Config;

fn get_sorted_children(cfg: &Config, node: &DecryptedFileMetadata) -> Vec<DecryptedFileMetadata> {
    let mut children = get_children(&cfg, node.id).unwrap_or_else(|err| {
        println!("error while retrieving file's children: {:#?}", err);
        return Vec::new();
    });
    
    children.sort_by(|a, b| a.decrypted_name.to_lowercase().cmp(&b.decrypted_name.to_lowercase()));

    return children
}

fn print_branch(cfg: &Config, file_leaf: &DecryptedFileMetadata, children: &Vec<DecryptedFileMetadata>, branch: &str, crotch: &str, twig: &str ) -> String {
    let mut sub_tree = format!("{}{}{}\n", branch, twig, file_leaf.decrypted_name);
    let mut next_branch = branch.to_string().clone();
    next_branch.push_str(crotch);

    let num_children = children.len();

    for (count, child) in children.iter().enumerate() {
        let mut next_crotch = "".to_string();
        let next_children = get_sorted_children(cfg, child);

        let sub_children = next_children.len() > 0;
        let last_child = count == num_children - 1;

        if sub_children {
            next_crotch.push_str( if last_child {"    "} else {"│   "} );
        }

        let next_twig = if last_child {"└── "} else {"├── "};

        sub_tree.push_str( &print_branch(cfg, child, &next_children, &next_branch, &next_crotch, next_twig));
    };

    return sub_tree;
}

pub fn tree() -> CliResult<()> {
    account()?;
    let cfg = config()?;
    let root = get_root(&cfg).unwrap();
    let children = get_sorted_children(&cfg, &root);

    println!("{}", print_branch(&cfg, &root, &children, "", "", ""));
    
    return Ok(())
}