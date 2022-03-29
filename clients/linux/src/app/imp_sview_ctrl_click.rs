use gtk::prelude::*;

impl super::App {
    pub fn handle_sview_ctrl_click(
        &self, g: &gtk::GestureClick, x: i32, y: i32, sview: &sv5::View,
    ) {
        let (buf_x, buf_y) = sview.window_to_buffer_coords(gtk::TextWindowType::Text, x, y);
        let iter = match sview.iter_at_location(buf_x, buf_y) {
            Some(iter) => iter,
            None => return,
        };

        let mut start = iter;
        start.backward_visible_line();
        start.forward_visible_line();

        let mut end = iter;
        end.forward_visible_line();

        let buf = sview.buffer().downcast::<sv5::Buffer>().unwrap();
        let text = buf.text(&start, &end, false);

        let uri_regex = regex::Regex::new(r"\[.*]\(([a-zA-z]+://)(.*)\)").unwrap();
        let index = iter.line_index();

        for captures in uri_regex.captures_iter(text.as_str()) {
            let whole = match captures.get(0) {
                Some(whole) => whole,
                None => return,
            };

            let loc = whole.start()..whole.end();
            if loc.contains(&(index as usize)) {
                let scheme = captures.get(1).map(|scheme| scheme.as_str()).unwrap();
                let content = captures.get(2).unwrap().as_str();
                let uri = format!("{}{}", scheme, content);

                match scheme {
                    "lb://" => self.open_file_from_id_str(content),
                    _ => gtk::show_uri(Some(&self.window), &uri, g.current_event_time()),
                }

                return;
            }
        }
    }
}
