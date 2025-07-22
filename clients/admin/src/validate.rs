use lb::blocking::Lb;

use crate::Res;

pub fn account(lb: &Lb, username: String) -> Res<()> {
    println!("Validating {username}...");

    let validation_failures = lb.admin_validate_account(&username)?;
    for failure in validation_failures.tree_validation_failures {
        println!("tree validation failure: {failure:?}");
    }
    for failure in validation_failures.documents_missing_content {
        println!("document missing content: {failure:?}");
    }
    for failure in validation_failures.documents_missing_size {
        println!("document missing size: {failure:?}");
    }

    Ok(())
}

pub fn server(core: &Lb) -> Res<()> {
    let validation_failures = core.admin_validate_server()?;
    println!("{validation_failures:#?}");
    Ok(())
}
