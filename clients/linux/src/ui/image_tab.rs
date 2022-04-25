use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

glib::wrapper! {
    pub struct ImageTab(ObjectSubclass<imp::ImageTab>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible;
}

impl ImageTab {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("failed to create ImageTab")
    }

    pub fn set_picture(&self, pic: &gtk::Picture) {
        self.imp().cntr.append(pic);
    }
}

impl Default for ImageTab {
    fn default() -> Self {
        Self::new()
    }
}

mod imp {
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    #[derive(Debug, Default)]
    pub struct ImageTab {
        pub cntr: gtk::Box,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ImageTab {
        const NAME: &'static str = "ImageTab";
        type Type = super::ImageTab;
        type ParentType = gtk::Widget;

        fn class_init(c: &mut Self::Class) {
            c.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for ImageTab {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            self.cntr.set_parent(obj);
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.cntr.unparent();
        }
    }

    impl WidgetImpl for ImageTab {}
}
