use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

glib::wrapper! {
    pub struct Tab(ObjectSubclass<imp::Tab>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible;
}

impl Tab {
    pub fn new(id: lb::Uuid) -> Self {
        glib::Object::new(&[("id", &id.to_string())]).expect("failed to create Tab")
    }

    pub fn id(&self) -> lb::Uuid {
        self.imp().id.get()
    }

    pub fn set_content<W: IsA<gtk::Widget>>(&self, w: &W) {
        self.imp().content.set_child(Some(w));
    }

    pub fn content<T: IsA<gtk::Widget>>(&self) -> Option<T> {
        self.imp()
            .content
            .child()
            .and_then(|w| w.downcast::<T>().ok())
    }

    pub fn tab_label(&self) -> &TabLabel {
        &self.imp().label
    }

    pub fn set_name(&self, name: &str) {
        self.imp().label.text_lbl.set_text(name);
    }

    pub fn name(&self) -> String {
        self.imp().label.text_lbl.text().to_string()
    }
}

#[derive(Debug)]
pub struct TabLabel {
    text_lbl: gtk::Label,
    pub close_btn: gtk::Button,
    pub cntr: gtk::Box,
}

impl Default for TabLabel {
    fn default() -> Self {
        let text_lbl = gtk::Label::new(None);

        let close_btn = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .build();
        close_btn.add_css_class("flat");

        let cntr = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        cntr.append(&text_lbl);
        cntr.append(&close_btn);

        Self { text_lbl, close_btn, cntr }
    }
}

mod imp {
    use std::cell::Cell;

    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    #[derive(Debug, Default)]
    pub struct Tab {
        pub id: Cell<lb::Uuid>,
        pub label: super::TabLabel,
        pub content: gtk::Overlay,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Tab {
        const NAME: &'static str = "Tab";
        type Type = super::Tab;
        type ParentType = gtk::Widget;

        fn class_init(c: &mut Self::Class) {
            // The layout manager determines how child widgets are laid out.
            c.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for Tab {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            self.content.set_parent(obj);
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
            self.content.unparent();
        }
    }

    impl WidgetImpl for Tab {}
}
