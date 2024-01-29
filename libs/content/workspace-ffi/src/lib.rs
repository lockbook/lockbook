use egui::{Pos2, Rect};
use egui_editor::input::canonical::{Location, Region};
use egui_editor::offset_types::{DocCharOffset, RelCharOffset};
use egui_wgpu_backend::wgpu;
use egui_wgpu_backend::wgpu::CompositeAlphaMode;
use lb_external_interface::lb_rs::Uuid;
use std::ffi::{c_char, CString};
use std::ptr::null;
use std::time::Instant;
use std::{iter, ptr};
use workspace::tab::markdown;

mod cursor_icon;

use crate::cursor_icon::CCursorIcon;
#[cfg(not(any(target_os = "ios", target_os = "macos")))]
use serde::Serialize;
use workspace::output::WsOutput;
use workspace::workspace::Workspace;

#[cfg(target_vendor = "apple")]
pub mod apple;

#[cfg(target_os = "android")]
pub mod android;

#[repr(C)]
#[derive(Debug)]
pub struct UITextSelectionRects {
    pub size: i32,
    pub rects: *const CRect,
}

impl Default for UITextSelectionRects {
    fn default() -> Self {
        UITextSelectionRects { size: 0, rects: null() }
    }
}

/// https://developer.apple.com/documentation/uikit/uitextrange
#[repr(C)]
#[derive(Debug, Default)]
pub struct CTextRange {
    /// used to represent a non-existent state of this struct
    pub none: bool,
    pub start: CTextPosition,
    pub end: CTextPosition,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct CTextPosition {
    /// used to represent a non-existent state of this struct
    pub none: bool,
    pub pos: usize, // represents a grapheme index
}

#[repr(C)]
#[derive(Debug)]
pub enum CTextLayoutDirection {
    Right = 2,
    Left = 3,
    Up = 4,
    Down = 5,
}

#[repr(C)]
#[derive(Debug)]
pub struct CPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Debug)]
pub enum CTextGranularity {
    Character = 0,
    Word = 1,
    Sentence = 2,
    Paragraph = 3,
    Line = 4,
    Document = 5,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct CRect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[repr(C)]
pub struct WgpuWorkspace {
    pub start_time: Instant,

    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub adapter: wgpu::Adapter,

    pub rpass: egui_wgpu_backend::RenderPass,
    pub screen: egui_wgpu_backend::ScreenDescriptor,

    pub context: egui::Context,
    pub raw_input: egui::RawInput,

    pub from_host: Option<String>,

    pub workspace: Workspace,
}

#[derive(Debug)]
#[repr(C)]
pub struct FfiWorkspaceResp {
    selected_file: CUuid,
    doc_created: CUuid,

    msg: *mut c_char,
    syncing: bool,
    refresh_files: bool,

    new_folder_btn_pressed: bool,

    #[cfg(target_os = "ios")]
    text_updated: bool,

    #[cfg(target_os = "ios")]
    selection_updated: bool,

    #[cfg(target_os = "ios")]
    selection_in_tab_title: bool,
}

impl Default for FfiWorkspaceResp {
    fn default() -> Self {
        Self {
            selected_file: Default::default(),
            doc_created: Default::default(),
            msg: ptr::null_mut(),
            syncing: Default::default(),
            refresh_files: Default::default(),
            new_folder_btn_pressed: Default::default(),
            #[cfg(target_os = "ios")]
            text_updated: Default::default(),
            #[cfg(target_os = "ios")]
            selection_updated: Default::default(),
            #[cfg(target_os = "ios")]
            selection_in_tab_title: false,
        }
    }
}

impl From<WsOutput> for FfiWorkspaceResp {
    fn from(value: WsOutput) -> Self {
        Self {
            selected_file: value.selected_file.unwrap_or_default().into(),
            msg: CString::new(value.status.message).unwrap().into_raw(),
            syncing: value.status.syncing,
            refresh_files: value.sync_done
                || value.file_renamed.is_some()
                || value.file_created.is_some(),
            doc_created: match value.file_created {
                Some(Ok(f)) => {
                    if f.is_document() {
                        f.id.into()
                    } else {
                        Uuid::nil().into()
                    }
                }
                _ => Uuid::nil().into(),
            },
            new_folder_btn_pressed: value.new_folder_clicked,
            #[cfg(target_os = "ios")]
            text_updated: false,
            #[cfg(target_os = "ios")]
            selection_updated: false,
            #[cfg(target_os = "ios")]
            selection_in_tab_title: true,
        }
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[repr(C)]
#[derive(Debug)]
pub struct IntegrationOutput {
    pub workspace_resp: FfiWorkspaceResp,
    pub redraw_in: u64,
    pub copied_text: *mut c_char,
    pub url_opened: *mut c_char,
    pub cursor: CCursorIcon,
}

impl Default for IntegrationOutput {
    fn default() -> Self {
        Self {
            redraw_in: Default::default(),
            workspace_resp: Default::default(),
            copied_text: ptr::null_mut(),
            url_opened: ptr::null_mut(),
            cursor: Default::default(),
        }
    }
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
#[derive(Debug, Default, Serialize)]
pub struct IntegrationOutput {
    pub redraw_in: u64,
    pub editor_response: EditorResponse,
}

impl Into<(DocCharOffset, DocCharOffset)> for CTextRange {
    fn into(self) -> (DocCharOffset, DocCharOffset) {
        (self.start.pos.into(), self.end.pos.into())
    }
}

impl Into<(RelCharOffset, RelCharOffset)> for CTextRange {
    fn into(self) -> (RelCharOffset, RelCharOffset) {
        (self.start.pos.into(), self.end.pos.into())
    }
}

impl Into<Region> for CTextRange {
    fn into(self) -> Region {
        Region::BetweenLocations { start: self.start.into(), end: self.end.into() }
    }
}

impl Into<Location> for CTextPosition {
    fn into(self) -> Location {
        Location::DocCharOffset(self.pos.into())
    }
}

impl WgpuWorkspace {
    pub fn frame(&mut self, show_tabs: bool) -> IntegrationOutput {
        let mut out = IntegrationOutput::default();
        self.configure_surface();
        let output_frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                // This error occurs when the app is minimized on Windows.
                // Silently return here to prevent spamming the console with:
                // "The underlying surface has changed, and therefore the swap chain must be updated"
                eprintln!("wgpu::SurfaceError::Outdated");
                return out;
            }
            Err(e) => {
                eprintln!("Dropped frame with error: {}", e);
                return out;
            }
        };
        let output_view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // can probably use run
        self.set_egui_screen();
        self.raw_input.time = Some(self.start_time.elapsed().as_secs_f64());
        self.context.begin_frame(self.raw_input.take());
        out.workspace_resp = self.workspace.draw(&self.context).into();
        let full_output = self.context.end_frame();
        if !full_output.platform_output.copied_text.is_empty() {
            // todo: can this go in output?
            out.copied_text = CString::new(full_output.platform_output.copied_text)
                .unwrap()
                .into_raw();
        }

        #[cfg(target_os = "ios")]
        {
            if let Some(markdown) = self.workspace.current_tab_markdown() {
                out.workspace_resp.text_updated = markdown.editor.text_updated;
                out.workspace_resp.selection_updated = (markdown.editor.scroll_area_offset
                    != markdown.editor.old_scroll_area_offset)
                    || markdown.editor.selection_updated;
                out.workspace_resp.selection_in_tab_title = self
                    .workspace
                    .current_tab()
                    .and_then(|tab| Some(tab.rename.is_some()))
                    .unwrap_or(false)
            }
        }

        if let Some(url) = full_output.platform_output.open_url {
            out.url_opened = CString::new(url.url).unwrap().into_raw();
        }

        out.cursor = full_output.platform_output.cursor_icon.into();

        let paint_jobs = self.context.tessellate(full_output.shapes);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("encoder") });

        let tdelta: egui::TexturesDelta = full_output.textures_delta;
        self.rpass
            .add_textures(&self.device, &self.queue, &tdelta)
            .expect("add texture ok");

        self.rpass
            .update_buffers(&self.device, &self.queue, &paint_jobs, &self.screen);
        // Record all render passes.
        self.rpass
            .execute(
                &mut encoder,
                &output_view,
                &paint_jobs,
                &self.screen,
                Some(wgpu::Color::BLACK),
            )
            .unwrap();
        // Submit the commands.
        self.queue.submit(iter::once(encoder.finish()));

        // Redraw egui
        output_frame.present();

        self.rpass
            .remove_textures(tdelta)
            .expect("remove texture ok");

        out.redraw_in = full_output.repaint_after.as_millis() as u64;
        out
    }

    pub fn set_egui_screen(&mut self) {
        self.raw_input.screen_rect = Some(Rect {
            min: Pos2::ZERO,
            max: Pos2::new(
                self.screen.physical_width as f32 / self.screen.scale_factor,
                self.screen.physical_height as f32 / self.screen.scale_factor,
            ),
        });
        self.raw_input.pixels_per_point = Some(self.screen.scale_factor);
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        // todo: is this really fine?
        // from here: https://github.com/hasenbanck/egui_example/blob/master/src/main.rs#L65
        self.surface.get_capabilities(&self.adapter).formats[0]
    }

    pub fn configure_surface(&self) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format(),
            width: self.screen.physical_width,
            height: self.screen.physical_height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        self.surface.configure(&self.device, &surface_config);
    }
}
#[repr(C)]
#[derive(Debug, Default)]
pub struct CUuid([u8; 16]);

impl From<Uuid> for CUuid {
    fn from(value: Uuid) -> Self {
        Self { 0: value.into_bytes() }
    }
}

impl From<CUuid> for Uuid {
    fn from(value: CUuid) -> Self {
        Uuid::from_bytes(value.0)
    }
}
