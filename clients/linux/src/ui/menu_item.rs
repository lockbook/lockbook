use gtk::prelude::*;

pub struct MenuItemBuilder {
    icon: Option<String>,
    label: String,
    accel: Option<String>,
    action: Option<String>,
    closes_popover: Option<gtk::Popover>,
}

impl MenuItemBuilder {
    pub fn new() -> Self {
        Self { icon: None, label: "".to_string(), accel: None, action: None, closes_popover: None }
    }

    pub fn icon(mut self, name: &str) -> Self {
        self.icon = Some(name.to_string());
        self
    }

    pub fn label(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    pub fn accel(mut self, accel: &str) -> Self {
        self.accel = Some(accel.to_string());
        self
    }

    pub fn action(mut self, action: &str) -> Self {
        self.action = Some(action.to_string());
        self
    }

    pub fn popsdown(mut self, p: &gtk::Popover) -> Self {
        self.closes_popover = Some(p.clone());
        self
    }

    pub fn build(self) -> gtk::Button {
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 16);

        if let Some(icon_name) = self.icon {
            let icon = gtk::Image::builder()
                .icon_name(&icon_name)
                .pixel_size(16)
                .build();
            content.append(&icon);
        }

        content.append(&gtk::Label::new(Some(&self.label)));

        let accel = gtk::Label::builder()
            .label(&self.accel.unwrap_or_default())
            .halign(gtk::Align::End)
            .hexpand(true)
            .sensitive(false)
            .margin_start(12)
            .build();
        content.append(&accel);

        let btn = gtk::Button::builder()
            .child(&content)
            .css_classes(vec!["flat".to_string(), "menu-item".to_string()])
            .can_focus(false)
            .build();

        if let Some(action_name) = self.action {
            btn.set_action_name(Some(&action_name));
        }

        if let Some(p) = self.closes_popover {
            btn.connect_clicked(move |_| p.popdown());
        }

        btn
    }
}

pub fn menu_separator() -> gtk::Separator {
    gtk::Separator::builder()
        .orientation(gtk::Orientation::Horizontal)
        .margin_bottom(2)
        .margin_top(2)
        .build()
}
