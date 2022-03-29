use std::sync::mpsc;

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

glib::wrapper! {
    pub struct TextEditor(ObjectSubclass<imp::TextEditor>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible;
}

impl Default for TextEditor {
    fn default() -> Self {
        Self::new(lb::Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap())
    }
}

impl TextEditor {
    pub fn new(id: lb::Uuid) -> Self {
        glib::Object::new(&[("id", &id.to_string())]).expect("failed to create TextEditor")
    }

    pub fn id(&self) -> lb::Uuid {
        self.imp().id.get()
    }

    pub fn name(&self) -> String {
        self.imp().name.text().as_str().to_string()
    }

    pub fn tab_label(&self) -> &gtk::Label {
        &self.imp().name
    }

    pub fn set_name(&self, name: &str) {
        self.imp().name.set_text(name)
    }

    pub fn editor(&self) -> &sv5::View {
        &self.imp().editor
    }

    pub fn connect_edit_alert_chan(&self, change_tx: mpsc::Sender<lb::Uuid>) {
        let id = self.imp().id.get();
        self.imp().editor.buffer().connect_changed(move |_| {
            change_tx.send(id).unwrap();
        });
    }
}

mod imp {
    use std::cell::Cell;

    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use sv5::prelude::*;

    #[derive(Debug, Default)]
    pub struct TextEditor {
        pub id: Cell<lb::Uuid>,
        pub name: gtk::Label,
        pub editor: sv5::View,
        pub scroll: gtk::ScrolledWindow,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TextEditor {
        const NAME: &'static str = "TextEditor";
        type Type = super::TextEditor;
        type ParentType = gtk::Widget;

        fn class_init(c: &mut Self::Class) {
            // The layout manager determines how child widgets are laid out.
            c.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for TextEditor {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let ed = &self.editor;
            ed.set_wrap_mode(gtk::WrapMode::Word);
            ed.set_monospace(true);
            ed.set_left_margin(4);
            ed.set_tab_width(4);

            self.scroll.set_child(Some(ed));
            self.scroll.set_parent(obj);
        }

        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPS: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecString::new(
                    "id", // Name
                    "id", // Nickname
                    "id", // Short description
                    None, // Default value
                    glib::ParamFlags::READWRITE,
                )]
            });
            PROPS.as_ref()
        }

        fn set_property(
            &self, _obj: &Self::Type, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "id" => {
                    let id_str: String = value
                        .get()
                        .expect("The value needs to be of type `String`.");
                    self.id.set(lb::Uuid::parse_str(&id_str).unwrap());
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "id" => self.id.get().to_string().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.scroll.unparent();
        }
    }

    impl WidgetImpl for TextEditor {}
}
