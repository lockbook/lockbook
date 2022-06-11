use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

#[derive(Clone)]
pub struct ToggleGroup<T, F> {
    value: Rc<Cell<T>>,
    on_changed: Rc<RefCell<Option<F>>>,
    pub cntr: gtk::Box,
}

impl<T, F> ToggleGroup<T, F>
where
    T: Copy + Default + PartialEq + 'static,
    F: Fn(T) + 'static,
{
    pub fn with_buttons(btns: &[(&str, T)]) -> Self {
        let default_value = T::default();
        let value = Rc::new(Cell::new(default_value));

        let on_changed = Rc::new(RefCell::new(None::<F>));

        let toggle_group = gtk::ToggleButton::new();
        let size_group = gtk::SizeGroup::new(gtk::SizeGroupMode::Horizontal);

        let cntr = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        cntr.add_css_class("toggle_btn_group");
        cntr.set_margin_bottom(4);

        for (label, val) in btns {
            let toggle_btn = gtk::ToggleButton::builder()
                .group(&toggle_group)
                .can_focus(false)
                .label(label)
                .build();

            if *val == default_value {
                toggle_btn.set_active(true);
            }

            let value = value.clone();
            let btn_value = *val;
            let on_changed = on_changed.clone();
            toggle_btn.connect_clicked(move |_| {
                value.set(btn_value);
                if let Some(f) = &*on_changed.borrow() {
                    f(btn_value);
                }
            });

            size_group.add_widget(&toggle_btn);
            cntr.append(&toggle_btn);
        }

        Self { value, on_changed, cntr }
    }

    pub fn value(&self) -> T {
        self.value.get()
    }

    pub fn connect_changed(&self, f: F) {
        f(self.value());
        *self.on_changed.borrow_mut() = Some(f);
    }
}
