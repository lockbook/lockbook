use gtk::glib;
use gtk::subclass::prelude::*;

glib::wrapper! {
    pub struct TextEditor(ObjectSubclass<imp::TextEditor>)
        @extends gtk::Widget,
        @implements gtk::Accessible;
}

impl TextEditor {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("failed to create TextEditor")
    }

    pub fn editor(&self) -> &sv5::View {
        &self.imp().editor
    }
}

impl Default for TextEditor {
    fn default() -> Self {
        Self::new()
    }
}

mod imp {
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use sv5::prelude::*;

    #[derive(Debug, Default)]
    pub struct TextEditor {
        pub editor: sv5::View,
        pub scroll: gtk::ScrolledWindow,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TextEditor {
        const NAME: &'static str = "TextEditor";
        type Type = super::TextEditor;
        type ParentType = gtk::Widget;

        fn class_init(c: &mut Self::Class) {
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

        fn dispose(&self, _obj: &Self::Type) {
            self.scroll.unparent();
        }
    }

    impl WidgetImpl for TextEditor {}
}
