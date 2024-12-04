use crate::data::{Graph, LinkNode};

use egui::ahash::{HashMap, HashMapExt};
use egui::epaint::Shape;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::{f32, time::Instant};
use std::{thread, usize};

// The makes it the code runs faster making it into grids
struct Grid {
    cell_size: f32,
    grid: HashMap<(i32, i32), Vec<usize>>,
}

// The main reason for these are to make global variables that can be accessed through the whole code
pub struct KnowledgeGraphApp {
    pub graph: Graph,
    positions: Vec<egui::Pos2>,
    zoom_factor: f32,
    pan: Vec2,
    last_pan: Vec2,
    last_screen_size: egui::Vec2,
    cursor_loc: egui::Vec2,
    debug: String,
    graph_complete: bool,

    directional_links: HashMap<usize, Vec<usize>>,
    thread_positions: Arc<RwLock<Vec<Pos2>>>,
    frame_count: usize,
    fps: f32,
    last_fps_update: Instant,
}

impl Grid {
    fn new(cell_size: f32) -> Self {
        Self { cell_size, grid: HashMap::new() }
    }

    fn insert_node(&mut self, pos: egui::Pos2, index: usize) {
        let grid_pos = self.get_grid_pos(pos);
        self.grid
            .entry(grid_pos)
            .or_insert_with(Vec::new)
            .push(index);
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

impl KnowledgeGraphApp {
    pub fn new(graph: &mut Graph) -> Self {
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

            directional_links: HashMap::new(),

            thread_positions: Arc::new(RwLock::new(thread_positions)),
            frame_count: 0,
            fps: 0.0,
            last_fps_update: Instant::now(),
        }
    }

    pub fn build_directional_links(&mut self) {
        let mut directional_links = HashMap::new();

        for node in &self.graph {
            let mut directional = Vec::new();
            for &link in &node.links {
                if link < self.graph.len() {
                    if !self.graph[link].links.contains(&node.id) {
                        directional.push(link);
                    }
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
                clusters
                    .entry(node.cluster_id)
                    .or_insert_with(Vec::new)
                    .push(node.id);
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

                let is_largest = Some(cluster_id).unwrap() == largest_cluster_id;

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
            for (_i, &node_id) in unlinked_nodes.iter().enumerate() {
                let nocluster: Option<usize> = None;
                self.graph[node_id].cluster_id = nocluster;
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
        screen: Rect,
    ) {
        let center =
            Pos2::new((screen.max.x + screen.min.x) / 2.0, (screen.max.y + screen.min.y) / 2.0);
        let mut previous_postions: VecDeque<Vec<Pos2>> = VecDeque::new();
        let num_nodes = graph.len() as f32;
        let width = screen.max.x - screen.min.x;
        let height = screen.max.y - screen.min.y; // Spring and repulsion constants
        let k_spring = 0.005;
        let k_repel = 3.0;
        let c = 0.05; // Scaling factor for movement
        let max_movement = 100.0;

        // Gravity parameters
        let gravity_strength = 0.0001; // Adjust as needed

        for _n in 0..max_iterations {
            let cell_size = (width * height / num_nodes).sqrt();
            let mut grid = Grid::new(cell_size);

            // Read current positions
            let positions = {
                let pos_lock = thread_positions.read().unwrap();
                pos_lock.clone()
            };

            // Insert nodes into the grid for spatial partitioning
            for (i, &pos) in positions.iter().enumerate() {
                grid.insert_node(pos, i);
            }

            // Initialize forces
            let mut forces = vec![Vec2::ZERO; graph.len()];

            // Calculate repulsive forces
            for i in 0..graph.len() {
                let pos_i = positions[i];

                for cell in grid.get_neighboring_cells(pos_i) {
                    for &j in cell {
                        if i != j {
                            let delta = pos_i - positions[j];
                            let distance = delta.length().max(0.01);

                            // Repulsive force calculation (inverse quartic)
                            let repulsive_force = k_repel / (distance * distance / 20.0);
                            let repulsion = delta.normalized() * repulsive_force;

                            forces[i] += repulsion;
                            forces[j] -= repulsion;
                        }
                    }
                }
            }

            // Calculate attractive forces
            for node in graph {
                for &link in &node.links {
                    if link >= graph.len() {
                        continue;
                    }

                    let delta = positions[node.id] - positions[link];
                    let distance = delta.length().max(0.01);

                    // Attractive force calculation (custom formula)
                    let attractive_force = k_spring * distance * (distance / 20.0) as f32;
                    let attraction = delta.normalized() * attractive_force;

                    forces[node.id] -= attraction;
                    forces[link] += attraction;
                }
            }

            // Apply gravity to pull nodes toward the center
            for i in 0..graph.len() {
                let delta = positions[i] - center;
                let distance = delta.length();
                let gravity_force = delta.normalized() * (distance * gravity_strength);
                forces[i] -= gravity_force;
            }

            let mut new_positions = positions.clone();
            // Update positions based on forces
            for i in 0..graph.len() {
                let force_magnitude = forces[i].length();

                let movement = if force_magnitude > max_movement {
                    forces[i] * (max_movement / force_magnitude)
                } else {
                    forces[i]
                };
                new_positions[i] += movement * c;
            }

            // Write updated positions back to thread_positions

            let clone_positions = positions.clone();
            previous_postions.push_back(clone_positions);

            {
                let mut pos_lock = thread_positions.write().unwrap();
                *pos_lock = new_positions.clone();
            }

            // Calculate total change for convergence
            let total_change: f32 = forces.iter().map(|f| f.length()).sum();

            // Debugging: Print iteration and total change
            if _n % 1000 == 0 {}

            // Convergence check
            if total_change < 0.01 * num_nodes || _n >= max_iterations {
                break;
            }
            grid.clear();
        }
    }

    fn draw_graph(&mut self, ui: &mut egui::Ui, screen_size: egui::Vec2) {
        let screen = ui.available_rect_before_wrap();
        let center =
            Pos2::new((screen.max.x + screen.min.x) / 2.0, (screen.max.y + screen.min.y) / 2.0);
        // let center = Pos2::new(screen_size.x / 2.0, screen_size.y / 2.0);
        let radius = (15.0) / ((self.graph.len() as f32).sqrt() / 3.0).max(1.0);
        let positions = {
            let pos_lock = self.thread_positions.read().unwrap();
            pos_lock.clone()
        };
        ui.painter().circle(
            center,
            5.0,
            egui::Color32::DEBUG_COLOR,
            Stroke::new(1.0, egui::Color32::BLACK),
        );
        self.last_pan = self.last_pan + self.pan / self.zoom_factor;
        self.pan = Vec2::ZERO;

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
                // let center = center + panning;
                let panned = pos.to_vec2() + panning;
                let zoomed = center.to_vec2() + ((panned - (center.to_vec2())) * self.zoom_factor);
                (zoomed).to_pos2()
            })
            .collect();
        let mut hoveredvalue = self.graph.len() + 1;
        for (i, _node) in self.graph.iter().enumerate() {
            let size = node_sizes[i];
            let pos = transformed_positions[i];
            if node_sizes[i] > 5.0 && cursorin(self.cursor_loc, pos, size) {
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
                        && cursorin(self.cursor_loc, pos, size)
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
        for (i, node) in self.graph.iter().enumerate() {
            let rgb_color = Color32::from_rgb(
                (node.color[0] * 255.0) as u8,
                (node.color[1] * 255.0) as u8,
                (node.color[2] * 255.0) as u8,
            );

            let size = node_sizes[i];
            let mut text_color = Color32::BLACK;
            let mut text = node.title.clone();
            if node.title.ends_with(".md") {
                text_color = Color32::LIGHT_BLUE;
                text = node.title.trim_end_matches(".md").to_string();
            }

            if node.cluster_id.is_some() {
                let pos = transformed_positions[i];
                ui.painter().circle(
                    pos,
                    size,
                    rgb_color,
                    Stroke::new(0.75 * self.zoom_factor, text_color),
                );

                if size > 5.0 && cursorin(self.cursor_loc, pos, size) {
                    let font_id = egui::FontId::proportional(15.0 * (self.zoom_factor.sqrt())); // Adjust font size based on zoom
                    text_info =
                        Some((pos, egui::Align2::CENTER_CENTER, text, font_id, Color32::WHITE));
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
                        && cursorin(self.cursor_loc, pos, size)
                    {
                        draw_arrow(
                            ui.painter(),
                            pos,
                            target,
                            Color32::from_rgba_unmultiplied(66, 135, 245, 150), // Semi-transparent blue
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

    pub fn bidiretional(&mut self) {
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

        // Update the color of the current node
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

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.input(|i| {
            if !self.graph_complete {
                self.build_directional_links();
                self.bidiretional();
                self.label_clusters();
                self.label_subgraphs();
            }
            let rect = ui.available_rect_before_wrap();

            if self.in_rect(rect) {
                // if ui.rect_contains_pointer(ui.available_rect_before_wrap()) {
                self.zoom_factor *= i.zoom_delta();
                self.debug = (self.zoom_factor).to_string();
                let scroll = i.raw_scroll_delta.to_pos2();
                self.pan += (scroll).to_vec2();
                self.debug = (self.zoom_factor).to_string();
            }
        });

        let screen = ui.available_rect_before_wrap();
        ui.set_clip_rect(screen);
        // ui.painter()
        // .rect_filled(rect, 0.0, egui::Color32::DEBUG_COLOR);
        let screen_size = screen.max.to_vec2();

        if !self.graph_complete {
            self.initialize_positions(ui);
            self.graph_complete = true;

            let postioninfo = Arc::clone(&self.thread_positions);
            let graph = self.graph.clone();
            thread::spawn(move || {
                Self::apply_spring_layout(postioninfo, &graph, 2500000, screen);
            });
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
    }
}

fn draw_arrow(
    painter: &Painter, from: Pos2, to: Pos2, color: Color32, zoom_factor: f32, size: f32,
    self_size: f32,
) {
    let intersect = intersectstuff(from, to, size);
    let intersect2 = intersectstuff1(from, to, self_size);

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

fn cursorin(cursor: Vec2, center: Pos2, size: f32) -> bool {
    if cursor.x > (center.x - size) && (center.x + size) > cursor.x {
        if cursor.y > (center.y - size) && (center.y + size) > cursor.y {
            return true;
        }
    }
    false
}
fn intersectstuff(from: Pos2, to: Pos2, size: f32) -> Pos2 {
    let x = from.x - to.x;
    let y = from.y - to.y;
    let angle = (y / x).atan();
    let new_x = size * angle.cos();
    let new_y: f32 = size * angle.sin();
    let mut intersect = Pos2::new(new_x, new_y);
    if x < 0.0 {
        intersect = intersect;
    } else {
        intersect = Pos2::new(0.0, 0.0) - intersect.to_vec2();
    }
    intersect
}
fn intersectstuff1(from: Pos2, to: Pos2, size: f32) -> Pos2 {
    let x = from.x - to.x;
    let y = from.y - to.y;
    let angle = (y / x).atan();
    let new_x = size * angle.cos();
    let new_y: f32 = size * angle.sin();
    let mut intersect = Pos2::new(new_x, new_y);
    if x > 0.0 {
        intersect = intersect;
    } else {
        intersect = Pos2::new(0.0, 0.0) - intersect.to_vec2();
    }
    intersect
}
