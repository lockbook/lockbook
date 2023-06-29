use is_terminal::IsTerminal;

use crate::Drive;

impl Drive {
    pub fn import(&self) {
        if std::io::stdin().is_terminal() {
            panic!("to import an existing lockbook account, pipe your account string into this command, e.g.:\npbpaste | drive import");
        }

        let mut account_string = String::new();
        std::io::stdin()
            .read_line(&mut account_string)
            .expect("failed to read from stdin");
        account_string.retain(|c| !c.is_whitespace());

        println!("importing account...");
        self.c.import_account(&account_string).unwrap();

        println!("account imported!");
    }
}
