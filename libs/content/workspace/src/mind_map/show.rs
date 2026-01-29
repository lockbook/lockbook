use super::data::{
    DONE, Graph, LinkNode, URL_NAME_STORE, lockbook_data, start_extraction_names, stop_extraction,
};
use egui::ahash::{HashMap, HashMapExt};
use egui::epaint::Shape;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::{f32, thread};
use web_time::{Duration, Instant};

struct Grid {
    cell_size: f32,
    grid: HashMap<(i32, i32), Vec<usize>>,
}

pub struct MindMap {
    pub graph: Graph,
    positions: Vec<egui::Pos2>,
    zoom_factor: f32,
    pan: Vec2,
    last_pan: Vec2,
    last_screen_size: egui::Vec2,
    cursor_loc: egui::Vec2,
    debug: String,
    graph_complete: bool,
    linkless_nodes: Vec<bool>,
    directional_links: HashMap<usize, Vec<usize>>,
    thread_positions: Arc<RwLock<Vec<Pos2>>>,
    stop: Arc<RwLock<bool>>,
    frame_count: usize,
    fps: f32,
    last_fps_update: Instant,
    inside: Option<Uuid>,
    inside_found: bool,
    urls_complete: bool,
    names_uploaded: bool,
    url_titles: Vec<String>,
    touch_positions: HashMap<u64, Pos2>,
    _last_tap_time: Option<f64>,
}

impl Grid {
    fn new(cell_size: f32) -> Self {
        Self { cell_size, grid: HashMap::new() }
    }

    fn insert_node(&mut self, pos: egui::Pos2, index: usize) {
        let grid_pos = self.get_grid_pos(pos);
        self.grid.entry(grid_pos).or_default().push(index);
    }

    fn get_grid_pos(&self, pos: egui::Pos2) -> (i32, i32) {
        let x = (pos.x / self.cell_size).floor() as i32;
        let y = (pos.y / self.cell_size).floor() as i32;
        (x, y)
    }

    fn get_neighboring_cells(&self, pos: egui::Pos2) -> Vec<&Vec<usize>> {
        let grid_pos = self.get_grid_pos(pos);
        let mut neighboring_cells = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(cell) = self.grid.get(&(grid_pos.0 + dx, grid_pos.1 + dy)) {
                    neighboring_cells.push(cell);
                }
            }
        }
        neighboring_cells
    }

    fn clear(&mut self) {
        self.grid.clear();
    }
}

impl MindMap {
    pub fn new(core: &Lb) -> Self {
        let graph = lockbook_data(core);
        let positions = vec![egui::Pos2::ZERO; graph.len()];
        let thread_positions = vec![egui::Pos2::ZERO; graph.len()];
        Self {
            graph: graph.clone(),
            positions,
            zoom_factor: 1.0,
            pan: Vec2::ZERO,
            last_pan: Vec2::ZERO,
            last_screen_size: egui::Vec2::new(800.0, 600.0),
            cursor_loc: egui::Vec2::ZERO,
            debug: String::from("no single touch"),
            graph_complete: false,
            linkless_nodes: vec![false; graph.len()],
            directional_links: HashMap::new(),
            thread_positions: Arc::new(RwLock::new(thread_positions)),
            stop: Arc::new(RwLock::new(false)),
            frame_count: 0,
            fps: 0.0,
            last_fps_update: Instant::now(),
            inside: None,
            inside_found: false,
            urls_complete: false,
            names_uploaded: false,
            url_titles: vec!["".to_string(); graph.len()],
            touch_positions: HashMap::new(),
            _last_tap_time: None,
        }
    }

    pub fn build_directional_links(&mut self) {
        let mut directional_links = HashMap::new();

        for node in &self.graph {
            let mut directional = Vec::new();
            for &link in &node.links {
                if link < self.graph.len() && !self.graph[link].links.contains(&node.id) {
                    directional.push(link);
                }
            }
            directional_links.insert(node.id, directional);
        }

        self.directional_links = directional_links;
    }

    fn initialize_positions(&mut self, ui: &mut egui::Ui) {
        let screen = ui.available_rect_before_wrap();
        let main_center =
            Pos2::new((screen.max.x + screen.min.x) / 2.0, (screen.max.y + screen.min.y) / 2.0);

        let cluster_small_radius = 10.0;

        let mut positions_map = HashMap::new();
        let mut clusters: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
        let mut unlinked_nodes: Vec<usize> = Vec::new();

        for node in &self.graph {
            if node.links.is_empty() {
                unlinked_nodes.push(node.id);
            } else {
                clusters.entry(node.cluster_id).or_default().push(node.id);
            }
        }

        let mut largest_cluster_id: Option<usize> = None;
        let mut largest_cluster_size: usize = 0;

        for (cluster_id, node_ids) in &clusters {
            if node_ids.len() > largest_cluster_size {
                largest_cluster_size = node_ids.len();
                largest_cluster_id = *cluster_id;
            }
        }

        let num_multi_clusters = clusters.len();
        let main_circle_radius = 200.0;

        if num_multi_clusters > 0 {
            let angle_step_clusters = 2.0 * std::f32::consts::PI / num_multi_clusters as f32;

            for (cluster_id, node_ids) in clusters {
                let number_nodes = node_ids.len();
                let angle_step_nodes = 2.0 * std::f32::consts::PI / number_nodes as f32;
                let mut count: f32 = 0.0;

                let is_largest = cluster_id == largest_cluster_id;

                let cluster_center = if is_largest {
                    main_center
                } else {
                    let angle = cluster_id.unwrap() as f32 * angle_step_clusters;
                    Pos2::new(
                        main_center.x + main_circle_radius * angle.cos(),
                        main_center.y + main_circle_radius * angle.sin(),
                    )
                };

                for node_id in node_ids {
                    let node_angle = count * angle_step_nodes;
                    let node_pos = Pos2::new(
                        cluster_center.x + cluster_small_radius * node_angle.cos(),
                        cluster_center.y + cluster_small_radius * node_angle.sin(),
                    );
                    positions_map.insert(node_id, node_pos);
                    count += 1.0;
                }
            }
        }

        let total_outer_nodes = unlinked_nodes.len();

        if total_outer_nodes > 0 {
            for &node_id in unlinked_nodes.iter() {
                let nocluster: Option<usize> = None;
                self.linkless_nodes[node_id] = true;
                self.graph[node_id].cluster_id = nocluster;
                let node_pos = Pos2::new(main_circle_radius * 3.0, main_circle_radius * 3.0);
                positions_map.insert(node_id, node_pos);
            }
        }

        self.positions = (0..self.graph.len())
            .map(|i| *positions_map.get(&i).unwrap_or(&main_center))
            .collect();
        {
            let mut threadinfo = self.thread_positions.write().unwrap();
            *threadinfo = self.positions.clone();
        }
    }

    fn apply_spring_layout(
        thread_positions: Arc<RwLock<Vec<Pos2>>>, graph: &[LinkNode], max_iterations: usize,
        screen: Rect, stop: Arc<RwLock<bool>>, linkless_node: Vec<bool>,
    ) {
        let center =
            Pos2::new((screen.max.x + screen.min.x) / 2.0, (screen.max.y + screen.min.y) / 2.0);
        let num_nodes = graph.len() as f32;
        let width = screen.max.x - screen.min.x;
        let height = screen.max.y - screen.min.y;
        let k_spring = 0.005;
        let k_repel = 3.0;
        let c = 0.05;
        let max_movement = 100.0;

        let gravity_strength = 0.0001;

        for _n in 0..max_iterations {
            let cell_size = (width * height / num_nodes).sqrt();
            let mut grid = Grid::new(cell_size);

            let positions = {
                let pos_lock = thread_positions.read().unwrap();
                pos_lock.clone()
            };

            for (i, &pos) in positions.iter().enumerate() {
                grid.insert_node(pos, i);
            }

            let mut forces = vec![Vec2::ZERO; graph.len()];

            for i in 0..graph.len() {
                if linkless_node[i] {
                    continue;
                }

                let pos_i = positions[i];

                for cell in grid.get_neighboring_cells(pos_i) {
                    for &j in cell {
                        if i != j && !linkless_node[j] {
                            let delta = pos_i - positions[j];
                            let distance = delta.length().max(0.01);

                            let repulsive_force = k_repel / (distance * distance / 20.0);
                            let repulsion = delta.normalized() * repulsive_force;

                            forces[i] += repulsion;
                            forces[j] -= repulsion;
                        }
                    }
                }
            }

            for node in graph {
                if linkless_node[node.id] {
                    continue;
                }

                for &link in &node.links {
                    if link >= graph.len() || linkless_node[link] {
                        continue;
                    }

                    let delta = positions[node.id] - positions[link];
                    let distance = delta.length().max(0.01);

                    let attractive_force = k_spring * distance * (distance / 20.0);
                    let attraction = delta.normalized() * attractive_force;

                    forces[node.id] -= attraction;
                    forces[link] += attraction;
                }
            }

            for i in 0..graph.len() {
                if linkless_node[i] {
                    continue;
                }

                let delta = positions[i] - center;
                let distance = delta.length();
                let gravity_force = delta.normalized() * (distance * gravity_strength);
                forces[i] -= gravity_force;
            }

            let mut new_positions = positions.clone();
            for i in 0..graph.len() {
                if linkless_node[i] {
                    continue;
                }

                let force_magnitude = forces[i].length();

                let movement = if force_magnitude > max_movement {
                    forces[i] * (max_movement / force_magnitude)
                } else {
                    forces[i]
                };
                new_positions[i] += movement * c;
            }

            {
                let mut pos_lock = thread_positions.write().unwrap();
                *pos_lock = new_positions.clone();
            }

            if _n >= max_iterations {
                break;
            }
            let stop = {
                let stop2 = stop.read().unwrap();
                *stop2
            };
            if stop {
                stop_extraction(true);
                // println!("stoped and close");
                break;
            }

            grid.clear();
        }
    }

    fn draw_graph(&mut self, ui: &mut egui::Ui, screen_size: egui::Vec2) {
        // println!("running");
        let screen = ui.available_rect_before_wrap();
        ui.painter()
            .rect_filled(screen, 0., ui.visuals().extreme_bg_color);

        let center =
            Pos2::new((screen.max.x + screen.min.x) / 2.0, (screen.max.y + screen.min.y) / 2.0);
        let radius = (15.0) / ((self.graph.len() as f32).sqrt() / 3.0).max(1.0);
        let positions = {
            let pos_lock = self.thread_positions.read().unwrap();
            pos_lock.clone()
        };
        if DONE.load(Ordering::SeqCst) && !self.names_uploaded {
            let info = &URL_NAME_STORE.lock().unwrap().clone();
            let completed = info.iter().any(|item| item.found);
            if completed {
                // println!("done2");
                self.urls_complete = true;
                self.populate_url_titles();
            }
        }
        self.last_pan += self.pan / self.zoom_factor;
        self.pan = Vec2::ZERO;
        let is_dark_mode = ui.ctx().style().visuals.dark_mode;
        let text_color: egui::Color32 = if is_dark_mode { Color32::WHITE } else { Color32::BLACK };
        let base_size = radius;
        let k = 1.0;
        let mut drawingstuf: Option<(usize, &LinkNode)> = None;
        let node_sizes: Vec<f32> = self
            .graph
            .iter()
            .map(|node| {
                let n = node.links.len() as f32;
                base_size + k * (n + 3.0).sqrt() * self.zoom_factor
            })
            .collect();
        let transformed_positions: Vec<Pos2> = positions
            .iter()
            .map(|pos| {
                let panning = self.last_pan;
                let panned = pos.to_vec2() + panning;
                let zoomed = center.to_vec2() + ((panned - (center.to_vec2())) * self.zoom_factor);
                (zoomed).to_pos2()
            })
            .collect();
        let mut hoveredvalue = self.graph.len() + 1;
        for (i, _node) in self.graph.iter().enumerate() {
            let size = node_sizes[i];
            let pos = transformed_positions[i];
            if node_sizes[i] > 5.0 && circle_contains(self.cursor_loc, pos, size) {
                hoveredvalue = i;
            }
        }
        for (i, node) in self.graph.iter().enumerate() {
            for &link in &node.links {
                if let Some(&target_pos) = transformed_positions.get(link) {
                    let size = node_sizes[i];
                    let pos = transformed_positions[i];
                    let target = target_pos;

                    if self.has_directed_link(node.id, self.graph[link].id)
                        && node_sizes[i] > 5.0
                        && circle_contains(self.cursor_loc, pos, size)
                    {
                        drawingstuf = Some((i, node));
                    } else if link == hoveredvalue {
                    } else {
                        ui.painter().line_segment(
                            [pos, target],
                            Stroke::new(1.0 * self.zoom_factor, Color32::GRAY),
                        );
                    }
                }
            }
        }

        let mut text_info: Option<(Pos2, Align2, String, FontId, Color32)> = None;
        self.inside_found = false;
        for (i, node) in self.graph.iter().enumerate() {
            let rgb_color = Color32::from_rgb(
                (node.color[0] * 255.0) as u8,
                (node.color[1] * 255.0) as u8,
                (node.color[2] * 255.0) as u8,
            );

            let size = node_sizes[i];
            let mut outline_color = Color32::BLACK;
            let mut text = node.title.clone();
            if node.title.ends_with(".md") {
                outline_color = Color32::LIGHT_BLUE;
                text = node.title.trim_end_matches(".md").to_string();

                if text.ends_with(")") {
                    text = text.trim_end_matches(")").to_string();
                }
            } else if self.names_uploaded {
                text = self.url_titles[i].clone();
            }
            text = truncate_after_second_punct(&text);
            if node.cluster_id.is_some() {
                let pos = transformed_positions[i];
                ui.painter().circle(
                    pos,
                    size,
                    rgb_color,
                    Stroke::new(0.75 * self.zoom_factor, outline_color),
                );

                if size > 5.0 && circle_contains(self.cursor_loc, pos, size) {
                    self.inside = node.file_id;
                    self.inside_found = true;
                    let font_id = egui::FontId::proportional(15.0 * (self.zoom_factor.sqrt()));
                    text_info = Some((pos, egui::Align2::CENTER_CENTER, text, font_id, text_color));
                }
            }
        }
        if let Some((i, node)) = drawingstuf {
            for &link in &node.links {
                if let Some(&target_pos) = transformed_positions.get(link) {
                    let size = node_sizes[i];
                    let pos = transformed_positions[i];
                    let target = target_pos;
                    let target_size = node_sizes[link];

                    if self.has_directed_link(node.id, self.graph[link].id)
                        && node_sizes[i] > 5.0
                        && circle_contains(self.cursor_loc, pos, size)
                    {
                        draw_arrow(
                            ui.painter(),
                            pos,
                            target,
                            Color32::from_rgba_unmultiplied(66, 135, 245, 150),
                            self.zoom_factor,
                            target_size,
                            size,
                        );
                    }
                }
            }
        }
        if let Some((pos, anchor, text, font_id, text_color)) = text_info {
            ui.painter().text(pos, anchor, text, font_id, text_color);
        }

        self.last_screen_size = screen_size;
    }

    fn has_directed_link(&self, from_node: usize, to_node: usize) -> bool {
        if let Some(links) = self.directional_links.get(&from_node) {
            links.contains(&to_node)
        } else {
            false
        }
    }

    fn populate_url_titles(&mut self) {
        let info = URL_NAME_STORE.lock().unwrap();
        for item in info.iter() {
            self.url_titles[item.id] = item.name.clone();
        }
        self.names_uploaded = true;
        // println!("{:?} this is some stuff", self.url_titles);
    }

    pub fn label_subgraphs(&mut self) {
        let mut bluecol = 1.0;
        let mut redcol = 0.1;
        let mut greencol = 0.5;

        for i in 0..self.graph.len() {
            if self.graph[i].color[2] == 0.0 {
                if self.graph[i].links.is_empty() {
                    self.graph[i].color = [1.0, 1.0, 1.0];
                } else {
                    self.dfs(i, bluecol, redcol, greencol);

                    bluecol = (bluecol * 0.7 + 0.2) % 1.0;
                    redcol = (redcol * 1.5 + 0.3) % 1.0;
                    greencol = (greencol * 1.3 + 0.4) % 1.0;
                }
            }
        }
    }

    pub fn label_clusters(&mut self) {
        let mut node_ids: Vec<usize> = Vec::new();

        for node in &self.graph {
            node_ids.push(node.id);
        }
        let mut count = 1;
        for node_id in node_ids {
            self.clusters(node_id, count);
            count += 1;
        }
    }

    fn clusters(&mut self, node_id: usize, cluster_id: usize) {
        if self.graph[node_id].cluster_id.is_some() {
            return;
        }

        self.graph[node_id].cluster_id = Some(cluster_id);

        let links = self.graph[node_id].links.clone();

        for link in links {
            if link != node_id {
                self.clusters(link, cluster_id);
            }
        }
    }

    pub fn bidirectional(&mut self) {
        let clonedgraph: &Graph = &self.graph.clone();
        for nodes in clonedgraph {
            let node: usize = nodes.id;
            for link in &nodes.links {
                if !clonedgraph[*link].links.contains(&node) {
                    self.graph[*link].links.push(node)
                }
            }
        }
    }

    fn dfs(&mut self, node_id: usize, col: f32, redcol: f32, greencol: f32) {
        let links_to_visit: Vec<usize> = {
            let node = &self.graph[node_id];
            node.links
                .iter()
                .filter_map(|&id| if self.graph[id].color[2] == 0.0 { Some(id) } else { None })
                .collect()
        };

        self.graph[node_id].color[0] = redcol;
        self.graph[node_id].color[1] = greencol;
        self.graph[node_id].color[2] = col;

        for id in links_to_visit {
            self.graph[id].color[0] = redcol;
            self.graph[id].color[1] = greencol;
            self.graph[id].color[2] = col;
            self.dfs(id, col, redcol, greencol);
        }
    }
    fn in_rect(&mut self, rect: Rect) -> bool {
        let min_range = rect.min;
        let max_range = rect.max;
        (min_range.x < self.cursor_loc.x)
            && self.cursor_loc.x < max_range.x
            && (min_range.y < self.cursor_loc.y)
            && self.cursor_loc.y < max_range.y
    }

    pub fn stop(&mut self) {
        // println!("in stop in mindmap");
        self.graph_complete = true;
        {
            let mut stop_lock = self.stop.write().unwrap();
            *stop_lock = true;
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Uuid> {
        let mut conditions = false;
        // println!("new version");

        // Do your graph-related building if necessary.
        if !self.graph_complete {
            self.build_directional_links();
            self.bidirectional(); // corrected from "bidiretional"
            self.label_clusters();
            self.label_subgraphs();
        }
        // Get the available rect.
        let rect = ui.available_rect_before_wrap();

        // If the current context’s rect is within our target...
        if self.in_rect(rect) {
            // Clone the current input events so we can iterate over them.
            let events = ui.input(|i| i.events.clone());
            for event in events {
                // println!("touch event");
                if let egui::Event::Touch { id, pos, phase, .. } = event {
                    // Process the touch event only if the touch is inside our rect.
                    let key = id.0;
                    if rect.contains(pos) {
                        match phase {
                            egui::TouchPhase::Start => {
                                // Save the starting position for this touch.
                                self.touch_positions.insert(key, pos);
                            }
                            egui::TouchPhase::Move => {
                                if let Some(prev_pos) = self.touch_positions.get(&key) {
                                    self.pan += pos - *prev_pos;

                                    // println!("Touch {:?} moved by {:?}", id, self.pan);
                                    // Update the stored position.
                                    self.touch_positions.insert(key, pos);
                                }
                            }
                            egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                                // Remove the touch tracking when it ends.
                                self.touch_positions.remove(&key);
                            }
                        }
                    }
                }
            }

            // Handle zoom and panning events.
            ui.input(|i| {
                self.zoom_factor *= i.zoom_delta();
                self.debug = self.zoom_factor.to_string();
                let scroll = i.raw_scroll_delta.to_pos2();
                self.pan += scroll.to_vec2();
                // You could update debug with pan if you wish.
            });
        }
        const _DOUBLE_TAP_THRESHOLD: f64 = 0.3;

        ui.input(|i| {
            // Touch platforms: require a double tap.
            #[cfg(any(target_os = "ios", target_os = "android"))]
            {
                if i.pointer.any_click() && self.inside_found {
                    // Use the current time from egui’s input state.
                    let now = i.time;
                    if let Some(_last_tap) = self._last_tap_time {
                        // Check if the new tap is within the double-tap time window.
                        if now - _last_tap < _DOUBLE_TAP_THRESHOLD {
                            // Double tap detected! Trigger the action.
                            conditions = true;
                            self.inside_found = false;
                            self._last_tap_time = None; // Reset for future detections.
                        } else {
                            // Too much time passed; treat this tap as the first tap.
                            self._last_tap_time = Some(now);
                        }
                    } else {
                        // First tap recorded; wait for the second tap.
                        self._last_tap_time = Some(now);
                    }
                }
            }

            // Non-touch platforms: trigger on a single click.
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            {
                if i.pointer.any_click() && self.inside_found {
                    conditions = true;
                    self.inside_found = false;
                }
            }
        });
        // Process a click event (if any) from pointer input.
        // ui.input(|i| {
        //     if i.pointer.any_click() && self.inside_found {
        //         conditions = true;
        //         self.inside_found = false;
        //     }
        // });

        if conditions {
            if let Some(val) = self.inside {
                return Some(val);
            }
        }

        // {
        //     let mut stop_write = self.stop.write().unwrap();
        //     *stop_write = stop;
        // }

        let screen = ui.available_rect_before_wrap();
        ui.set_clip_rect(screen);
        let screen_size = screen.max.to_vec2();

        if !self.graph_complete {
            self.initialize_positions(ui);
            self.graph_complete = true;
            let linkess = self.linkless_nodes.clone();
            let positioninfo = Arc::clone(&self.thread_positions);
            let stop = Arc::clone(&self.stop);
            let graph = self.graph.clone();
            // thread::spawn(move || {
            //     Self::apply_spring_layout(positioninfo, &graph, 2500000, screen, stop, linkess);
            // });
        }
        if !self.urls_complete {
            // thread::spawn(start_extraction_names);
            self.urls_complete = true;
        }

        self.draw_graph(ui, screen_size);
        ui.ctx().request_repaint();

        self.frame_count += 1;
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_fps_update);
        if elapsed >= Duration::from_secs(1) {
            self.fps = self.frame_count as f32 / elapsed.as_secs_f32();
            self.frame_count = 0;
            self.last_fps_update = now;
        }
        if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
            self.cursor_loc = cursor.to_vec2();
        }
        None
    }
}
fn draw_arrow(
    painter: &Painter, from: Pos2, to: Pos2, color: Color32, zoom_factor: f32, size: f32,
    self_size: f32,
) {
    let intersect = intersect_stuff(from, to, size, true);
    let intersect2 = intersect_stuff(from, to, self_size, false);

    let to = to - intersect.to_vec2();
    let from = from - intersect2.to_vec2();
    let arrow_length = 6.0 * zoom_factor;
    let arrow_width = 4.0 * zoom_factor;

    let arrow_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 255);

    let direction = to - from;
    let distance = direction.length();

    if distance == 0.0 {
        return;
    }

    let dir = direction / distance;

    let arrow_base = to - dir * arrow_length;

    let perp = Vec2::new(-dir.y, dir.x);

    let arrow_p1 = arrow_base + perp * (arrow_width / 2.0);
    let arrow_p2 = arrow_base - perp * (arrow_width / 2.0);

    painter.line_segment([from, arrow_base], Stroke::new(1.0 * zoom_factor, arrow_color));

    let points = vec![to, arrow_p1, arrow_p2];

    painter.add(Shape::convex_polygon(points, arrow_color, Stroke::new(0.0, color)));
}

fn circle_contains(p: Vec2, center: Pos2, size: f32) -> bool {
    if p.x > (center.x - size)
        && (center.x + size) > p.x
        && p.y > (center.y - size)
        && (center.y + size) > p.y
    {
        return true;
    }
    false
}
fn intersect_stuff(from: Pos2, to: Pos2, size: f32, zero: bool) -> Pos2 {
    let x = from.x - to.x;
    let y = from.y - to.y;
    let angle = (y / x).atan();
    let new_x = size * angle.cos();
    let new_y: f32 = size * angle.sin();
    let mut intersect = Pos2::new(new_x, new_y);
    if zero {
        if x < 0.0 {
        } else {
            intersect = Pos2::new(0.0, 0.0) - intersect.to_vec2();
        }
    } else if x > 0.0 {
    } else {
        intersect = Pos2::new(0.0, 0.0) - intersect.to_vec2();
    }

    intersect
}

fn remove_words_with_backslash(text: &str) -> String {
    // If the entire text is just one word (no spaces), return it as-is.
    if text.split_whitespace().count() == 1 {
        return text.to_string();
    }

    // Otherwise, filter out any words that contain '\' or '/'.
    text.split_whitespace()
        .filter(|word| !word.contains('\\') && !word.contains('/'))
        .collect::<Vec<&str>>()
        .join(" ")
}

fn rearrange_last_word(text: &str) -> String {
    // Helper closure to check if a candidate is exactly one word with no trailing spaces.
    let is_exact_single_word = |s: &str| {
        let trimmed = s.trim();
        // Check that there's exactly one word...
        trimmed.split_whitespace().count() == 1
        // ...and that there are no trailing spaces after trimming.
        && s == s.trim_end()
    };

    // Check for the " · " separator first.
    if let Some(pos) = text.rfind(" · ") {
        let separator = " · ";
        let first_part = &text[..pos];
        let last_part = &text[pos + separator.len()..];
        if is_exact_single_word(last_part) {
            return format!("{}{}{}", last_part.trim(), separator, first_part.trim());
        }
    }
    // Otherwise, check for the " - " separator.
    if let Some(pos) = text.rfind(" - ") {
        let separator = " - ";
        let first_part = &text[..pos];
        let last_part = &text[pos + separator.len()..];
        if is_exact_single_word(last_part) {
            return format!("{}{}{}", last_part.trim(), separator, first_part.trim());
        }
    }

    text.to_string()
}

fn truncate_after_second_punct(text: &str) -> String {
    let mut punct_count = 0;
    let mut number_count = 0;
    let mut number_space = 0;
    let mut return_meet = false;
    let text = rearrange_last_word(text);
    let text = remove_words_with_backslash(&text);
    for (i, c) in text.char_indices() {
        if c.is_numeric() {
            number_count += 1;
        }
        if c == ' ' {
            number_space += 1;
        }
        if c == '?' || c == '/' || c == ',' {
            punct_count += 1;
        }
        return_meet = return_meet || punct_count > 2 || number_count > 3 || number_space > 5;
        if return_meet {
            // When our condition is met, truncate the text up to and including this character
            return text[..i].to_owned();
            // Now, if the truncated text ends with a separator and a word, rearrange it
        }
    }
    text
    // If no truncation condition was met, still check the entire text
}
