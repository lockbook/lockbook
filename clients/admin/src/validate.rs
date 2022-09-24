use crate::Res;
use lockbook_core::Core;

pub fn account(core: &Core, username: String) -> Res<()> {
    println!("Validating server...");

    let validation_failures = core.admin_server_validate(&username)?;
    for failure in validation_failures.tree_validation_failures {
        println!("tree validation failure: {:?}", failure);
    }
    for failure in validation_failures.documents_missing_content {
        println!("document missing content: {:?}", failure);
    }
    for failure in validation_failures.documents_missing_size {
        println!("document missing size: {:?}", failure);
    }

    Ok(())
}
