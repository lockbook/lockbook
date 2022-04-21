use gtk::glib;
use gtk::prelude::*;

pub enum OnboardOp {
    CreateAccount { uname: String, api_url: String },
    ImportAccount(String),
}

#[derive(Clone)]
pub struct OnboardScreen {
    error_create: gtk::Label,
    error_import: gtk::Label,
    pub status: Status,
    stack: gtk::Stack,
    switcher: gtk::StackSwitcher,
    pub cntr: gtk::Box,
}

#[derive(Clone)]
pub struct Status {
    spinner: gtk::Spinner,
    title: gtk::Label,
    pub caption: gtk::Label,
    cntr: gtk::Box,
}

impl OnboardScreen {
    pub fn new(op_chan: &glib::Sender<OnboardOp>) -> Self {
        let heading = gtk::Label::builder()
            .css_classes(vec!["onboard-heading".to_string()])
            .margin_top(16)
            .margin_bottom(16)
            .label("Lockbook")
            .build();

        let stack = gtk::Stack::new();

        let error_create = gtk::Label::new(None);

        let uname_entry = gtk::Entry::new();
        uname_entry.set_placeholder_text(Some("Pick a username..."));
        uname_entry.connect_activate({
            let op_chan = op_chan.clone();

            move |entry| {
                let uname = entry.buffer().text();
                let api_url = std::env::var("API_URL")
                    .unwrap_or_else(|_| lb::DEFAULT_API_LOCATION.to_string());
                op_chan
                    .send(OnboardOp::CreateAccount { uname, api_url })
                    .unwrap();
            }
        });

        let create = gtk::Box::new(gtk::Orientation::Vertical, 0);
        create.append(&uname_entry);
        create.append(&error_create);
        stack.add_titled(&create, Some("create"), "Create Account");

        let error_import = gtk::Label::new(None);

        let acct_str_entry = gtk::Entry::new();
        acct_str_entry.set_placeholder_text(Some("Account string..."));
        acct_str_entry.connect_activate({
            let op_chan = op_chan.clone();

            move |entry| {
                let acct_str = entry.buffer().text();
                op_chan.send(OnboardOp::ImportAccount(acct_str)).unwrap();
            }
        });

        let import = gtk::Box::new(gtk::Orientation::Vertical, 0);
        import.append(&acct_str_entry);
        import.append(&error_import);
        stack.add_titled(&import, Some("import"), "Import Account");

        let status = Status::new();
        stack.add_named(&status.cntr, Some("status"));

        let switcher = gtk::StackSwitcher::builder()
            .stack(&stack)
            .margin_top(20)
            .margin_bottom(20)
            .build();

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 0);
        cntr.set_halign(gtk::Align::Center);
        cntr.set_valign(gtk::Align::Center);
        cntr.append(&super::logo(256));
        cntr.append(&heading);
        cntr.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        cntr.append(&switcher);
        cntr.append(&stack);

        Self { status, error_create, error_import, stack, switcher, cntr }
    }

    pub fn start(&self, title: &str) {
        self.status.title.set_text(title);
        self.switcher.set_sensitive(false);
        self.stack.set_visible_child_name("status");
        self.status.spinner.start();
        self.status.spinner.show();
    }

    pub fn stop(&self, back_to: &str) {
        self.status.spinner.stop();
        self.status.title.set_text("");
        self.stack.set_visible_child_name(back_to);
        self.switcher.set_sensitive(true);
    }

    pub fn handle_create_error(&self, err: lb::Error<lb::CreateAccountError>, uname: &str) {
        use lb::CreateAccountError::*;

        let txt = match err {
            lb::Error::UiError(err) => match err {
                UsernameTaken => format!("The username '{}' is already taken.", uname),
                InvalidUsername => format!("Invalid username '{}' ({}).", uname, UNAME_REQS),
                AccountExistsAlready => "An account already exists.".to_string(),
                CouldNotReachServer => "Unable to connect to the server.".to_string(),
                ServerDisabled => "The server is disabled.".to_string(),
                ClientUpdateRequired => "Client upgrade required.".to_string(),
            },
            lb::Error::Unexpected(msg) => msg,
        };

        self.error_create
            .set_markup(&format!("<span foreground=\"red\">{}</span>", txt));
        self.stack.set_visible_child_name("create");
    }

    pub fn handle_import_error(&self, err: lb::Error<lb::ImportAccountError>) {
        use lb::ImportAccountError::*;

        let txt = match err {
            lb::Error::UiError(err) => match err {
                AccountStringCorrupted => "Your account's private key is corrupted.",
                AccountExistsAlready => "An account already exists.",
                AccountDoesNotExist => "The account you tried to import does not exist.",
                UsernamePKMismatch => "The account private key does not match username.",
                CouldNotReachServer => "Unable to connect to the server.",
                ClientUpdateRequired => "Client upgrade required.",
            }
            .to_string(),
            lb::Error::Unexpected(msg) => msg,
        };

        self.set_import_err_msg(&txt);
    }

    pub fn set_import_err_msg(&self, msg: &str) {
        self.error_import
            .set_markup(&format!("<span foreground=\"red\">{}</span>", msg));
        self.stack.set_visible_child_name("import");
    }
}

impl Status {
    fn new() -> Self {
        let spinner = gtk::Spinner::new();
        let title = gtk::Label::new(None);
        let caption = gtk::Label::new(None);

        let cntr = gtk::Box::new(gtk::Orientation::Vertical, 16);
        cntr.append(&{
            let bx = gtk::Box::new(gtk::Orientation::Horizontal, 16);
            bx.set_halign(gtk::Align::Center);
            bx.append(&spinner);
            bx.append(&title);
            bx
        });
        cntr.append(&caption);

        Self { spinner, title, caption, cntr }
    }
}

const UNAME_REQS: &str = "letters and numbers only";
