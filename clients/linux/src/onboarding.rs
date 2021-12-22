use gdk_pixbuf::Pixbuf as GdkPixbuf;
use gtk::prelude::*;
use gtk::Orientation::{Horizontal, Vertical};

use crate::app::LbApp;
use crate::backend::LbSyncMsg;
use crate::error::{LbErrKind, LbErrTarget, LbError, LbResult};
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
        let import = OnboardingInput::new(m, Msg::ImportAccount, "Account string...");
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

    fn set_status(&self, caption: &str) {
        self.bottom.set_visible_child_name("status");
        self.status.start(caption);
    }

    fn sync_progress(&self, s: &LbSyncMsg) {
        let status = format!("Syncing :: {} ({}/{})", s.name, s.index, s.total);
        self.status.status.set_text(&status);
    }

    fn error_create(&self, msg: &str) {
        self.bottom.set_visible_child_name("input");
        self.create.error(msg);
        self.status.stop();
    }

    fn error_import(&self, msg: &str) {
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

pub fn create(lb: &LbApp, name: String) -> LbResult<()> {
    lb.gui.onboarding.set_status("Creating account...");

    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    rx.attach(
        None,
        glib::clone!(@strong lb => move |result: LbResult<()>| {
            match result {
                Ok(_) => lb.gui.show_account_screen(),
                Err(err) => match err.kind() {
                    LbErrKind::User => lb.gui.onboarding.error_create(err.msg()),
                    LbErrKind::Program => lb.messenger.send_err_dialog("creating account", err),
                },
            }
            glib::Continue(false)
        }),
    );

    std::thread::spawn(glib::clone!(@strong lb.core as c => move || {
        tx.send(c.create_account(&name)).unwrap();
    }));

    Ok(())
}

pub fn import(lb: &LbApp, acct_str: String) -> LbResult<()> {
    lb.gui.onboarding.set_status("Importing account...");

    // Create a channel to receive and process the result of importing the account. If there is any
    // error, it's shown on the import screen. Otherwise, account syncing will start.
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    rx.attach(
        None,
        glib::clone!(@strong lb => move |result: LbResult<()>| {
            match result {
                Ok(_) => import_account_sync(&lb),
                Err(err) => lb.gui.onboarding.error_import(err.msg()),
            }
            glib::Continue(false)
        }),
    );

    // In a separate thread, import the account and send the result down the channel.
    std::thread::spawn(glib::clone!(
        @strong lb.core as c,
        @strong lb.messenger as m
        => move || {
            if let Err(err) = tx.send(c.import_account(&acct_str)) {
                m.send_err_dialog("sending import result", LbError::fmt_program_err(err));
            }
        }
    ));

    Ok(())
}

fn import_account_sync(lb: &LbApp) {
    // Create a channel to receive and process any account sync progress updates.
    let (sync_chan, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    rx.attach(
        None,
        glib::clone!(@strong lb => move |msgopt: Option<LbSyncMsg>| {
            // If there is some message, show it. If not, syncing is done, so the
            // account screen is shown.
            if let Some(msg) = msgopt {
                lb.gui.onboarding.sync_progress(&msg)
            } else {
                lb.gui.show_account_screen();
                std::thread::spawn(glib::clone!(@strong lb.messenger as m => move || {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    m.send(Msg::RefreshUsageStatus);
                }));
            }
            glib::Continue(true)
        }),
    );

    // In a separate thread, start syncing the account. Pass the sync channel which will be
    // used to receive progress updates as indicated above.
    std::thread::spawn(glib::clone!(
        @strong lb.core as c,
        @strong lb.messenger as m
        => move || {
            if let Err(err) = c.sync(sync_chan) {
                match err.target() {
                    LbErrTarget::Dialog => m.send_err_dialog("syncing", err),
                    LbErrTarget::StatusPanel => m.send_err_status_panel(err.msg()),
                }
            }
        }
    ));
}

pub const LOGO: &[u8] = include_bytes!("../res/lockbook-onboarding-pixdata");
