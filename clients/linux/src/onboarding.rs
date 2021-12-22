use gdk_pixbuf::Pixbuf as GdkPixbuf;
use gtk::prelude::*;
use gtk::Orientation::{Horizontal, Vertical};

use crate::backend::LbSyncMsg;
use crate::messages::{Messenger, Msg};

pub struct Screen {
    create: OnboardingInput,
    import: OnboardingInput,
    status: OnboardingStatus,
    bottom: gtk::Stack,
    pub cntr: gtk::Box,
}

impl Screen {
    pub fn new(m: &Messenger) -> Self {
        let create = OnboardingInput::new(m, Msg::CreateAccount, "Pick a username...");
        let import = OnboardingInput::new(m, Msg::ImportAccount, "Private key...");
        let status = OnboardingStatus::new();

        let bottom = gtk::Stack::new();
        bottom.add_named(&Self::inputs(&create, &import), "input");
        bottom.add_named(&status.cntr, "status");

        let cntr = gtk::Box::new(Vertical, 48);
        cntr.set_valign(gtk::Align::Center);
        cntr.set_halign(gtk::Align::Center);
        cntr.add(&Self::top());
        cntr.add(&Self::sep());
        cntr.add(&bottom);

        Self {
            create,
            import,
            status,
            bottom,
            cntr,
        }
    }

    fn top() -> gtk::Box {
        let heading = gtk::Label::new(Some("Lockbook"));
        gtk::WidgetExt::set_widget_name(&heading, "onboarding_heading");

        let cntr = gtk::Box::new(Horizontal, 32);
        cntr.set_halign(gtk::Align::Center);
        cntr.add(&gtk::Image::from_pixbuf(Some(
            &GdkPixbuf::from_inline(LOGO, false).unwrap(),
        )));
        cntr.add(&heading);
        cntr
    }

    fn sep() -> gtk::Box {
        let hr = gtk::Separator::new(Horizontal);
        hr.set_size_request(512, -1);
        gtk::WidgetExt::set_widget_name(&hr, "onboarding_hr");

        let sep = gtk::Box::new(Horizontal, 0);
        sep.set_center_widget(Some(&hr));
        sep
    }

    fn inputs(create: &OnboardingInput, import: &OnboardingInput) -> gtk::Box {
        let stack = gtk::Stack::new();
        stack.add_titled(&create.cntr, "create", "Create Account");
        stack.add_titled(&import.cntr, "import", "Import Account");

        let switcher = gtk::StackSwitcher::new();
        switcher.set_stack(Some(&stack));
        switcher.set_margin_bottom(32);

        let cntr = gtk::Box::new(Vertical, 0);
        cntr.set_halign(gtk::Align::Center);
        cntr.add(&switcher);
        cntr.add(&stack);
        cntr
    }

    pub fn set_status(&self, caption: &str) {
        self.bottom.set_visible_child_name("status");
        self.status.start(caption);
    }

    pub fn sync_progress(&self, s: &LbSyncMsg) {
        let status = format!("Syncing :: {} ({}/{})", s.name, s.index, s.total);
        self.status.status.set_text(&status);
    }

    pub fn error_create(&self, msg: &str) {
        self.bottom.set_visible_child_name("input");
        self.create.error(msg);
        self.status.stop();
    }

    pub fn error_import(&self, msg: &str) {
        self.bottom.set_visible_child_name("input");
        self.import.error(msg);
        self.status.stop();
    }
}

struct OnboardingInput {
    error: gtk::Label,
    cntr: gtk::Box,
}

impl OnboardingInput {
    fn new(m: &Messenger, msg: fn(String) -> Msg, desc: &str) -> Self {
        let m = m.clone();
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some(desc));
        entry.connect_activate(move |entry| {
            let value = entry.get_buffer().get_text();
            m.send(msg(value));
        });

        let error = gtk::Label::new(None);
        error.set_margin_top(16);
        gtk::WidgetExt::set_widget_name(&error, "onboarding_error");

        let cntr = gtk::Box::new(Vertical, 0);
        cntr.add(&entry);
        cntr.add(&error);

        Self { error, cntr }
    }

    fn error(&self, txt: &str) {
        self.cntr.show();
        self.error.set_text(txt);
    }
}

struct OnboardingStatus {
    spinner: gtk::Spinner,
    caption: gtk::Label,
    status: gtk::Label,
    cntr: gtk::Box,
}

impl OnboardingStatus {
    fn new() -> Self {
        let spinner = gtk::Spinner::new();
        spinner.set_size_request(24, 24);

        let caption = gtk::Label::new(None);
        gtk::WidgetExt::set_widget_name(&caption, "onboarding_status_caption");

        let status = gtk::Label::new(None);

        let cntr = gtk::Box::new(Vertical, 32);
        cntr.add(&{
            let bx = gtk::Box::new(Horizontal, 16);
            bx.set_halign(gtk::Align::Center);
            bx.add(&spinner);
            bx.add(&caption);
            bx
        });
        cntr.add(&status);

        Self {
            spinner,
            caption,
            status,
            cntr,
        }
    }

    fn start(&self, txt: &str) {
        self.cntr.show_all();
        self.caption.set_text(txt);
        self.spinner.start();
    }

    fn stop(&self) {
        self.spinner.stop();
    }
}

pub const LOGO: &[u8] = include_bytes!("../res/lockbook-onboarding-pixdata");
