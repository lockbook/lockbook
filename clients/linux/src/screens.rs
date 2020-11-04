use std::rc::Rc;

use gtk::prelude::*;
use gtk::Stack as GtkStack;

use crate::account::AccountScreen;
use crate::backend::LbCore;
use crate::intro::IntroScreen;
use crate::messages::{Messenger, Msg};
use crate::settings::Settings;

pub struct Screens {
    pub intro: IntroScreen,
    pub account: Rc<AccountScreen>,
    pub cntr: GtkStack,
}

impl Screens {
    pub fn new(m: &Messenger, s: &Settings) -> Self {
        let intro = IntroScreen::new(m);
        let account = AccountScreen::new(m, &s);

        let cntr = GtkStack::new();
        cntr.add_named(&intro.cntr, "intro");
        cntr.add_named(&account.cntr, "account");

        Self {
            cntr,
            intro,
            account: Rc::new(account),
        }
    }

    pub fn init(&self, core: &LbCore, m: &Messenger) {
        match core.account() {
            Ok(acct) => match acct {
                Some(_) => self.show_account(core),
                None => self.show_intro(),
            },
            Err(err) => m.send(Msg::UnexpectedErr(
                "Unable to load account".to_string(),
                err,
            )),
        }
    }

    fn show_intro(&self) {
        self.intro.cntr.show_all();
        self.set("intro");
    }

    pub fn show_account(&self, core: &LbCore) {
        self.account.cntr.show_all();
        self.account.fill(&core);
        self.set("account");
    }

    fn set(&self, name: &str) {
        self.cntr.set_visible_child_name(name);
    }
}
