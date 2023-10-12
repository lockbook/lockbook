use crate::Res;
use lb::Core;

pub fn account(core: &Core, username: String) -> Res<()> {
    println!("Validating {username}...");

    let validation_failures = core.admin_validate_account(&username)?;
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

pub fn server(core: &Core) -> Res<()> {
    let validation_failures = core.admin_validate_server()?;
    println!("{:#?}", validation_failures);
    Ok(())
}
