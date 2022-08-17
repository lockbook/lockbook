use std::process::Command;

pub trait CommandRunner {
    fn assert_success(&mut self);
}

impl CommandRunner for Command {
    fn assert_success(&mut self) {
        if !self.status().unwrap().success() {
            panic!()
        }
    }
}
