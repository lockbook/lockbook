use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use gtk::glib;
use gtk::prelude::*;

use crate::bg;
use crate::settings::Settings;
use crate::ui;

impl super::App {
    pub fn activate(api: Arc<dyn lb::Api>, a: &gtk::Application) {
        let writeable_path =
            env::var("LOCKBOOK_PATH").unwrap_or(format!("{}/.lockbook", env::var("HOME").unwrap()));

        let window = gtk::ApplicationWindow::new(a);
        window.set_title(Some("Lockbook"));

        if let Err(err) = check_and_perform_migrations(&api) {
            let msg = format!("checking and performing migrations: {}", err);
            show_launch_error(&window, &msg);
            return;
        }

        let settings = match Settings::from_data_dir(&writeable_path) {
            Ok(s) => Arc::new(RwLock::new(s)),
            Err(err) => {
                let msg = format!("unable to read settings file: {}", err);
                show_launch_error(&window, &msg);
                return;
            }
        };

        let lang_mngr = match new_language_manager(&writeable_path) {
            Ok(lm) => lm,
            Err(err) => {
                let msg = format!("unable to write custom language file: {}", err);
                show_launch_error(&window, &msg);
                return;
            }
        };

        let (bg_op_tx, bg_op_rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let bg_state = bg::State::new(bg_op_tx);

        let (onboard_op_tx, onboard_op_rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let onboard = ui::OnboardScreen::new(&onboard_op_tx);

        let (account_op_tx, account_op_rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let account = ui::AccountScreen::new(
            account_op_tx,
            lang_mngr,
            &settings.read().unwrap().hidden_tree_cols,
        );

        let app = Self { api, settings, window, onboard, account, bg_state };

        app.add_app_actions(a);

        app.clone().listen_for_theme_changes();
        app.clone().listen_for_onboard_ops(onboard_op_rx);
        app.clone().listen_for_account_ops(account_op_rx);
        app.clone().listen_for_bg_ops(bg_op_rx);

        app.setup_window();
        app.window.present();
    }

    fn add_app_actions(&self, a: &gtk::Application) {
        {
            let app = self.clone();
            let new_doc = gio::SimpleAction::new("new-document", None);
            new_doc.connect_activate(move |_, _| app.new_file(lb::FileType::Document));
            a.add_action(&new_doc);
        }
        {
            let app = self.clone();
            let new_folder = gio::SimpleAction::new("new-folder", None);
            new_folder.connect_activate(move |_, _| app.new_file(lb::FileType::Folder));
            a.add_action(&new_folder);
        }
        {
            let app = self.clone();
            let open_file = gio::SimpleAction::new("open-file", Some(glib::VariantTy::STRING));
            open_file.connect_activate(move |_, param| {
                let param = param
                    .expect("there must be a uuid string for 'app.open-file'")
                    .get::<String>()
                    .expect("the 'app.open-file' parameter must be type String");
                let id = lb::Uuid::parse_str(&param).unwrap();
                app.open_file(id);
            });
            a.add_action(&open_file);
        }
        {
            let app = self.clone();
            let save_file = gio::SimpleAction::new("save-file", None);
            save_file.connect_activate(move |_, _| app.save_file(None));
            a.add_action(&save_file);
            a.set_accels_for_action("app.save-file", &["<Ctrl>S"]);
        }
        {
            let app = self.clone();
            let close_file = gio::SimpleAction::new("close-file", None);
            close_file.connect_activate(move |_, _| app.close_file());
            a.add_action(&close_file);
            a.set_accels_for_action("app.close-file", &["<Ctrl>W"]);
        }
        {
            let app = self.clone();
            let rename_file = gio::SimpleAction::new("rename-file", None);
            rename_file.connect_activate(move |_, _| app.rename_file());
            a.add_action(&rename_file);
        }
        {
            let app = self.clone();
            let del_files = gio::SimpleAction::new("delete-files", None);
            del_files.connect_activate(move |_, _| app.delete_files());
            a.add_action(&del_files);
        }
        {
            let app = self.clone();
            let exp_files = gio::SimpleAction::new("export-files", None);
            exp_files.connect_activate(move |_, _| app.export_files());
            a.add_action(&exp_files);
        }
        {
            let prompt_search = gio::SimpleAction::new("prompt-search", None);
            prompt_search.connect_activate(move |_, _| {});
            a.add_action(&prompt_search);
            a.set_accels_for_action("app.prompt-search", &["<Ctrl>space"]);
        }
        {
            let app = self.clone();
            let sync = gio::SimpleAction::new("sync", None);
            sync.connect_activate(move |_, _| app.perform_sync());
            a.add_action(&sync);
            a.set_accels_for_action("app.sync", &["<Alt>S"]);
        }
        {
            let app = self.clone();
            let open_settings = gio::SimpleAction::new("settings", None);
            open_settings.connect_activate(move |_, _| app.open_settings_dialog());
            a.add_action(&open_settings);
            a.set_accels_for_action("app.settings", &["<Ctrl>comma"]);
        }
        {
            let app = self.clone();
            let open_about = gio::SimpleAction::new("about", None);
            open_about.connect_activate(move |_, _| ui::about_dialog::open(&app.window));
            a.add_action(&open_about);
        }
    }

    fn listen_for_onboard_ops(self, onboard_op_rx: glib::Receiver<ui::OnboardOp>) {
        onboard_op_rx.attach(None, move |op| {
            use ui::OnboardOp::*;
            match op {
                CreateAccount(uname, url) => self.create_account(uname, url),
                ImportAccount(account_string) => self.import_account(account_string),
            }
            glib::Continue(true)
        });
    }

    fn listen_for_account_ops(self, account_op_rx: glib::Receiver<ui::AccountOp>) {
        account_op_rx.attach(None, move |op| {
            use ui::AccountOp::*;
            match op {
                TreeReceiveDrop(val, x, y) => self.tree_receive_drop(&val, x, y),
                TabSwitched(tab) => self.window.set_title(Some(&tab.name())),
                AllTabsClosed => self.window.set_title(Some("Lockbook")),
            }
            glib::Continue(true)
        });
    }

    fn listen_for_bg_ops(self, bg_op_rx: glib::Receiver<bg::Op>) {
        bg_op_rx.attach(None, move |op| {
            use bg::Op::*;
            match op {
                AutoSave(id) => self.save_file(Some(id)),
                AutoSync => self.perform_sync(),
            }
            glib::Continue(true)
        });
    }

    fn setup_window(&self) {
        let settings = self.settings.clone();
        self.window.connect_close_request(move |_| {
            if let Err(err) = settings.write().unwrap().to_file() {
                eprintln!("error: {}", err);
            }
            gtk::Inhibit(false)
        });

        self.window.set_titlebar(Some(&ui::header_bar::new()));
        self.window.set_default_size(900, 700);

        if self.settings.read().unwrap().window_maximize {
            self.window.maximize();
        }

        match self.api.account() {
            Ok(Some(_acct)) => self.init_account_screen(),
            Ok(None) => self.window.set_child(Some(&self.onboard.cntr)),
            Err(msg) => show_launch_error(&self.window, &msg),
        }
    }

    pub fn init_account_screen(&self) {
        match self.api.list_metadatas() {
            Ok(mut metas) => self.account.tree.populate(&mut metas),
            Err(err) => println!("{}", err), //todo
        }

        self.update_sync_status();
        self.bg_state.begin_work(&self.api, &self.settings);
        self.window.set_child(Some(&self.account.cntr))
    }
}

fn check_and_perform_migrations(api: &Arc<dyn lb::Api>) -> Result<(), String> {
    const STATE_REQ_CLEAN_MSG: &str =
        "Your local state cannot be migrated, please re-sync with a fresh client.";

    match api.db_state().map_err(|err| format!("{}", err))? {
        lb::DbState::ReadyToUse | lb::DbState::Empty => Ok(()),
        lb::DbState::StateRequiresClearing => Err(STATE_REQ_CLEAN_MSG.to_string()),
        lb::DbState::MigrationRequired => {
            println!("Local state requires migration! Performing migration now...");
            api.migrate_db().map_err(|err| match err {
                lb::Error::UiError(lb::MigrationError::StateRequiresCleaning) => {
                    STATE_REQ_CLEAN_MSG.to_string()
                }
                lb::Error::Unexpected(msg) => msg,
            })
        }
    }
}

fn new_language_manager(data_dir: &str) -> Result<sv5::LanguageManager, String> {
    let lang_dir = format!("{}/lang-specs", data_dir);
    let custom_lang = format!("{}/custom.lang", lang_dir);
    if !Path::new(&lang_dir).exists() {
        fs::create_dir(&lang_dir).map_err(|e| format!("{}", e))?;
        fs::write(custom_lang, include_bytes!("../../custom.lang"))
            .map_err(|e| format!("{}", e))?;
    }

    let lang_mngr = sv5::LanguageManager::default();
    let lang_paths = lang_mngr.search_path();
    let mut lang_paths = lang_paths
        .iter()
        .map(|path| path.as_str())
        .collect::<Vec<&str>>();
    lang_paths.push(&lang_dir);
    lang_mngr.set_search_path(&lang_paths);
    Ok(lang_mngr)
}

fn show_launch_error(win: &gtk::ApplicationWindow, msg: &str) {
    let err_lbl = ui::unexpected_error(msg);
    win.set_default_size(500, 300);
    win.set_child(Some(&err_lbl));
    win.connect_close_request(|_| {
        std::process::exit(1);
    });
    win.show();
}
