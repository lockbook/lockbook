use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

glib::wrapper! {
    pub struct ImageTab(ObjectSubclass<imp::ImageTab>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible;
}

impl ImageTab {
    pub fn new(id: lb::Uuid) -> Self {
        glib::Object::new(&[("id", &id.to_string())]).expect("failed to create ImageTab")
    }

    pub fn set_picture(&self, pic: &gtk::Picture) {
        self.imp().cntr.append(pic);
    }

    pub fn tab_label(&self) -> &gtk::Label {
        &self.imp().name
    }
}

impl super::Tab for ImageTab {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn page(&self) -> gtk::Widget {
        self.clone().upcast::<gtk::Widget>()
    }

    fn id(&self) -> lb::Uuid {
        self.imp().id.get()
    }

    fn set_name(&self, name: &str) {
        self.imp().name.set_text(name)
    }

    fn name(&self) -> String {
        self.imp().name.text().as_str().to_string()
    }
}

mod imp {
    use std::cell::Cell;

    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    #[derive(Debug, Default)]
    pub struct ImageTab {
        pub id: Cell<lb::Uuid>,
        pub name: gtk::Label,
        pub cntr: gtk::Box,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ImageTab {
        const NAME: &'static str = "ImageTab";
        type Type = super::ImageTab;
        type ParentType = gtk::Widget;

        fn class_init(c: &mut Self::Class) {
            // The layout manager determines how child widgets are laid out.
            c.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for ImageTab {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.cntr.set_parent(obj);
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
            self.cntr.unparent();
        }
    }

    impl WidgetImpl for ImageTab {}
}
