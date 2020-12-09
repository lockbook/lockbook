pub const BYTE: u64 = 1;
pub const KILOBYTE: u64 = BYTE * 1024;
pub const MEGABYTE: u64 = KILOBYTE * 1024;
pub const GIGABYTE: u64 = MEGABYTE * 1024;
pub const TERABYTE: u64 = GIGABYTE * 1024;

const KILOBYTE_PLUS_ONE: u64 = KILOBYTE + 1;
const MEGABYTE_PLUS_ONE: u64 = MEGABYTE + 1;
const GIGABYTE_PLUS_ONE: u64 = GIGABYTE + 1;
const TERABYTE_PLUS_ONE: u64 = TERABYTE + 1;

pub fn human_readable_bytes(v: u64) -> String {
    let (unit, abbr) = match v {
        0..=KILOBYTE => (BYTE, ""),
        KILOBYTE_PLUS_ONE..=MEGABYTE => (KILOBYTE, "K"),
        MEGABYTE_PLUS_ONE..=GIGABYTE => (MEGABYTE, "M"),
        GIGABYTE_PLUS_ONE..=TERABYTE => (GIGABYTE, "G"),
        TERABYTE_PLUS_ONE..=u64::MAX => (TERABYTE, "T"),
    };
    format!("{:.3} {}B", v as f64 / unit as f64, abbr)
}

pub mod gui {
    use gtk::prelude::ButtonExt;
    use gtk::prelude::ContainerExt;
    use gtk::prelude::IsA;
    use gtk::prelude::WidgetExt;
    use gtk::Adjustment as GtkAdjustment;
    use gtk::Button as GtkButton;
    use gtk::Clipboard as GtkClipboard;
    use gtk::ScrolledWindow as GtkScrolledWindow;
    use gtk::Widget as GtkWidget;

    pub fn clipboard_btn(txt: &str) -> GtkButton {
        let txt = txt.to_string();
        let btn = GtkButton::with_label("Copy to Clipboard");
        btn.connect_clicked(move |_| {
            GtkClipboard::get(&gdk::SELECTION_CLIPBOARD).set_text(&txt);
        });
        btn
    }

    pub fn scrollable<W: IsA<GtkWidget>>(widget: &W) -> GtkScrolledWindow {
        let sw = GtkScrolledWindow::new(None::<&GtkAdjustment>, None::<&GtkAdjustment>);
        sw.add(widget);
        sw
    }

    pub fn set_margin<W: IsA<GtkWidget>>(w: &W, v: i32) {
        set_marginx(w, v);
        set_marginy(w, v);
    }

    pub fn set_marginx<W: IsA<GtkWidget>>(w: &W, v: i32) {
        w.set_margin_start(v);
        w.set_margin_end(v);
    }

    pub fn set_marginy<W: IsA<GtkWidget>>(w: &W, v: i32) {
        w.set_margin_top(v);
        w.set_margin_bottom(v);
    }
}
