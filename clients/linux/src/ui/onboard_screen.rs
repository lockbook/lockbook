use gtk::glib;
use gtk::prelude::*;

pub enum OnboardOp {
    CreateAccount { uname: String, api_url: String },
    ImportAccount { account_string: String },
}

pub enum OnboardRoute {
    Create,
    Import,
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

impl OnboardScreen {
    pub fn new(op_chan: &glib::Sender<OnboardOp>) -> Self {
        let heading = gtk::Label::new(Some("Lockbook"));
        heading.add_css_class("onboard-heading");

        let uname_entry = gtk::Entry::builder()
            .placeholder_text("Pick a username...")
            .build();
        uname_entry.connect_activate({
            let op_chan = op_chan.clone();

            move |entry| {
                let uname = entry.text().to_string();
                let api_url = std::env::var("API_URL")
                    .unwrap_or_else(|_| lb::DEFAULT_API_LOCATION.to_string());
                op_chan
                    .send(OnboardOp::CreateAccount { uname, api_url })
                    .unwrap();
            }
        });

        let error_create = gtk::Label::builder().wrap(true).build();

        let create = gtk::Box::new(gtk::Orientation::Vertical, 0);
        create.append(&uname_entry);
        create.append(&error_create);

        let acct_str_entry = gtk::PasswordEntry::builder()
            .placeholder_text("Account string...")
            .show_peek_icon(true)
            .build();
        acct_str_entry.connect_activate({
            let op_chan = op_chan.clone();

            move |entry| {
                let account_string = entry.text().to_string();
                op_chan
                    .send(OnboardOp::ImportAccount { account_string })
                    .unwrap();
            }
        });

        let error_import = gtk::Label::builder().wrap(true).build();

        let import = gtk::Box::new(gtk::Orientation::Vertical, 0);
        import.append(&acct_str_entry);
        import.append(&error_import);

        let status = Status::new();

        let stack = gtk::Stack::new();
        stack.add_titled(&create, Some("create"), "Create Account");
        stack.add_titled(&import, Some("import"), "Import Account");
        stack.add_named(&status.cntr, Some("status"));

        let switcher = gtk::StackSwitcher::builder().stack(&stack).build();

        let cntr = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .halign(gtk::Align::Center)
            .margin_top(30)
            .spacing(20)
            .build();
        cntr.append(&super::logo(256));
        cntr.append(&heading);
        cntr.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        cntr.append(&switcher);
        cntr.append(&stack);

        Self { status, error_create, error_import, stack, switcher, cntr }
    }

    pub fn start(&self, which: OnboardRoute) {
        self.status.title.set_text(match which {
            OnboardRoute::Create => "Creating account...",
            OnboardRoute::Import => "Importing account...",
        });
        self.switcher.set_sensitive(false);
        self.stack.set_visible_child_name("status");
        self.status.spinner.start();
        self.status.spinner.show();
    }

    pub fn stop(&self, which: OnboardRoute) {
        self.status.spinner.stop();
        self.status.title.set_text("");
        self.stack.set_visible_child_name(match which {
            OnboardRoute::Create => "create",
            OnboardRoute::Import => "import",
        });
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

    pub fn handle_import_error(&self, err: lb::Error<lb::ImportError>) {
        use lb::ImportError::*;

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

#[derive(Clone)]
pub struct Status {
    spinner: gtk::Spinner,
    title: gtk::Label,
    pub caption: gtk::Label,
    cntr: gtk::Box,
}

impl Status {
    fn new() -> Self {
        let spinner = gtk::Spinner::new();
        let title = gtk::Label::new(None);
        let caption = gtk::Label::builder().wrap(true).build();

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
