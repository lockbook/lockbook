#[macro_export]
macro_rules! cloned_var_name {
    ($v:ident) => {
        $v
    };
    ($v:ident $( $_:ident )+) => {
        $v
    };
}

#[macro_export]
macro_rules! closure {
    ($( $( $vars:ident ).+ $( as $aliases:ident )? ),+ => $fn:expr) => {{
        $( let $crate::cloned_var_name!($( $aliases )? $( $vars )+)  = $( $vars ).+.clone(); )+
        $fn
    }};
}

pub mod gui {
    use gtk::prelude::ButtonExt;
    use gtk::prelude::ContainerExt;
    use gtk::prelude::IsA;
    use gtk::prelude::WidgetExt as GtkWidgetExt;
    use gtk::Adjustment as GtkAdjustment;
    use gtk::Align as GtkAlign;
    use gtk::Button as GtkButton;
    use gtk::Clipboard as GtkClipboard;
    use gtk::Container as GtkContainer;
    use gtk::Label as GtkLabel;
    use gtk::ScrolledWindow as GtkScrolledWindow;
    use gtk::Widget as GtkWidget;

    pub fn add<C: IsA<GtkContainer>, W: IsA<GtkWidget>>(cntr: &C, w: &W) {
        let mut contained = false;
        cntr.foreach(|child| {
            if child == w {
                contained = true;
            }
        });

        if !contained {
            cntr.add(w);
        }
    }

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

    pub fn set_widget_name<W: IsA<GtkWidget>>(w: &W, name: &str) {
        GtkWidgetExt::set_widget_name(w, name);
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

    pub fn text_right(txt: &str) -> GtkLabel {
        let l = GtkLabel::new(Some(txt));
        l.set_halign(GtkAlign::End);
        l.set_margin_end(4);
        l
    }

    pub fn text_left(txt: &str) -> GtkLabel {
        let l = GtkLabel::new(Some(txt));
        l.set_halign(GtkAlign::Start);
        l.set_margin_start(4);
        l
    }

    pub const RIGHT_CLICK: u32 = 3;
}
