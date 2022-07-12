use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use gtk::glib;
use gtk::prelude::*;

use crate::bg;
use crate::lbutil;
use crate::settings::Settings;
use crate::ui;

impl super::App {
    pub fn activate(core: Arc<lb::Core>, a: &gtk::Application) {
        let data_dir = lbutil::data_dir();

        let titlebar = ui::Titlebar::new();

        let overlay = gtk::Overlay::new();
        overlay.add_overlay(titlebar.search_result_area());

        let window = gtk::ApplicationWindow::new(a);
        window.set_child(Some(&overlay));

        let settings = match Settings::from_data_dir(&data_dir) {
            Ok(s) => Arc::new(RwLock::new(s)),
            Err(err) => {
                let msg = format!("unable to read settings file: {}", err);
                show_launch_error(&window, &msg);
                return;
            }
        };

        let lang_mngr = match new_language_manager(&data_dir) {
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

        let app = Self {
            sync_lock: Arc::new(Mutex::new(())),
            core,
            settings,
            window,
            overlay,
            titlebar,
            onboard,
            account,
            bg_state,
        };

        app.clone().listen_for_theme_changes();
        app.clone().listen_for_onboard_ops(onboard_op_rx);
        app.clone().listen_for_account_ops(account_op_rx);
        app.clone().listen_for_bg_ops(bg_op_rx);
        app.listen_for_search_ops();

        app.setup_window();
        app.window.present();
    }

    fn listen_for_onboard_ops(self, onboard_op_rx: glib::Receiver<ui::OnboardOp>) {
        onboard_op_rx.attach(None, move |op| {
            use ui::OnboardOp::*;
            match op {
                CreateAccount { uname, api_url } => self.create_account(uname, api_url),
                ImportAccount { account_string } => self.import_account(account_string),
            }
            glib::Continue(true)
        });
    }

    fn listen_for_account_ops(self, account_op_rx: glib::Receiver<ui::AccountOp>) {
        account_op_rx.attach(None, move |op| {
            use ui::AccountOp::*;
            match op {
                NewFile => self.prompt_new_file(),
                OpenFile(id) => self.open_file(id),
                RenameFile => self.rename_file(),
                DeleteFiles => self.delete_files(),
                ExportFiles => self.export_files(),
                CutFiles => self.cut_selected_files(),
                CopyFiles => self.copy_selected_files(),
                PasteFiles => self.paste_into_selected_file(),
                TreeReceiveDrop(val, x, y) => self.tree_receive_drop(&val, x, y),
                TabSwitched(tab) => self.titlebar.set_title(&tab.name()),
                AllTabsClosed => self.titlebar.set_title("Lockbook"),
                SviewCtrlClick { click, x, y, sview } => {
                    self.handle_sview_ctrl_click(&click, x, y, &sview)
                }
                SviewInsertFileList { id, buf, flist } => {
                    self.sview_insert_file_list(id, &buf, flist)
                }
                SviewInsertTexture { id, buf, texture } => {
                    self.sview_insert_texture(id, &buf, texture)
                }
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

        self.window.set_title(Some("Lockbook"));
        self.window.set_default_size(1000, 700);

        if self.settings.read().unwrap().window_maximize {
            self.window.maximize();
        }

        match lbutil::get_account(&self.core) {
            Ok(Some(_acct)) => self.init_account_screen(),
            Ok(None) => self.overlay.set_child(Some(&self.onboard.cntr)),
            Err(msg) => show_launch_error(&self.window, &msg),
        }
    }

    pub fn init_account_screen(&self) {
        self.window.set_titlebar(Some(&self.titlebar));

        match self.core.list_metadatas() {
            Ok(mut metas) => self.account.tree.populate(&mut metas),
            Err(err) => println!("{}", err), //todo
        }

        self.update_sync_status();
        self.bg_state.begin_work(&self.core, &self.settings);
        self.overlay.set_child(Some(&self.account.cntr));
        self.add_app_actions(&self.window.application().unwrap());
    }

    fn add_app_actions(&self, a: &gtk::Application) {
        {
            let app = self.clone();
            let save_file = gio::SimpleAction::new("new-file", None);
            save_file.connect_activate(move |_, _| app.prompt_new_file());
            a.add_action(&save_file);
            a.set_accels_for_action("app.new-file", &["<Ctrl>N"]);
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
            let prompt_search = gio::SimpleAction::new("open-search", None);
            prompt_search.connect_activate(move |_, _| app.open_search());
            a.add_action(&prompt_search);
            a.set_accels_for_action("app.open-search", &["<Ctrl>space", "<Ctrl>L"]);
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
