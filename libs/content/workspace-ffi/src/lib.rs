use egui::{Pos2, Rect};
use egui_editor::input::canonical::{Location, Region};
use egui_editor::offset_types::{DocCharOffset, RelCharOffset};
use egui_wgpu_backend::wgpu;
use egui_wgpu_backend::wgpu::CompositeAlphaMode;
use lb_external_interface::lb_rs::Uuid;
use std::ffi::{c_char, CString};
use std::ptr;
use std::time::Instant;

#[cfg(target_vendor = "apple")]
use std::iter;

mod cursor_icon;

#[cfg(not(target_vendor = "apple"))]
use serde::Serialize;

#[cfg(not(target_vendor = "apple"))]
use egui_editor::EditorResponse;

#[cfg(target_vendor = "apple")]
use crate::cursor_icon::CCursorIcon;

use workspace_rs::output::WsOutput;
use workspace_rs::workspace::Workspace;

#[cfg(target_vendor = "apple")]
pub mod apple;

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_vendor = "apple")]
#[repr(C)]
#[derive(Debug)]
pub struct UITextSelectionRects {
    pub size: i32,
    pub rects: *const CRect,
}

#[cfg(target_vendor = "apple")]
impl Default for UITextSelectionRects {
    fn default() -> Self {
        UITextSelectionRects { size: 0, rects: ptr::null() }
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

    // remember size last frame to detect resize
    pub surface_width: u32,
    pub surface_height: u32,

    pub rpass: egui_wgpu_backend::RenderPass,
    pub screen: egui_wgpu_backend::ScreenDescriptor,

    pub context: egui::Context,
    pub raw_input: egui::RawInput,

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
    tab_title_clicked: bool,
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
            tab_title_clicked: false,
        }
    }
}

impl From<WsOutput> for FfiWorkspaceResp {
    fn from(value: WsOutput) -> Self {
        Self {
            selected_file: value.selected_file.unwrap_or_default().into(),
            msg: CString::new(value.status.message).unwrap().into_raw(),
            syncing: value.status.syncing,
            refresh_files: value.sync_done.is_some()
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
            tab_title_clicked: value.tab_title_clicked,
        }
    }
}

#[cfg(target_vendor = "apple")]
#[repr(C)]
#[derive(Debug)]
pub struct IntegrationOutput {
    pub workspace_resp: FfiWorkspaceResp,
    pub redraw_in: u64,
    pub copied_text: *mut c_char,
    pub url_opened: *mut c_char,
    pub cursor: CCursorIcon,
}

#[cfg(target_vendor = "apple")]
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

#[cfg(not(target_vendor = "apple"))]
#[derive(Debug, Default, Serialize)]
pub struct IntegrationOutput {
    pub redraw_in: u64,
    pub editor_response: EditorResponse,
}

impl From<CTextRange> for (DocCharOffset, DocCharOffset) {
    fn from(value: CTextRange) -> Self {
        (value.start.pos.into(), value.end.pos.into())
    }
}

impl From<CTextRange> for (RelCharOffset, RelCharOffset) {
    fn from(value: CTextRange) -> Self {
        (value.start.pos.into(), value.end.pos.into())
    }
}

impl From<CTextRange> for Region {
    fn from(value: CTextRange) -> Self {
        Region::BetweenLocations { start: value.start.into(), end: value.end.into() }
    }
}

impl From<CTextPosition> for Location {
    fn from(value: CTextPosition) -> Self {
        Location::DocCharOffset(value.pos.into())
    }
}

impl WgpuWorkspace {
    #[cfg(target_vendor = "apple")]
    pub fn frame(&mut self) -> IntegrationOutput {
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

        #[cfg(target_os = "ios")]
        {
            if let Some(markdown) = self.workspace.current_tab_markdown() {
                out.workspace_resp.text_updated = markdown.editor.text_updated;
                out.workspace_resp.selection_updated = (markdown.editor.scroll_area_offset
                    != markdown.editor.old_scroll_area_offset)
                    || markdown.editor.selection_updated;
            }
        }

        if !full_output.platform_output.copied_text.is_empty() {
            // todo: can this go in output?
            out.copied_text = CString::new(full_output.platform_output.copied_text)
                .unwrap()
                .into_raw();
        }

        if let Some(url) = full_output.platform_output.open_url {
            out.url_opened = CString::new(url.url).unwrap().into_raw();
        }

        out.cursor = full_output.platform_output.cursor_icon.into();

        let paint_jobs = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);
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

    pub fn configure_surface(&mut self) {
        let resized = self.screen.physical_width != self.surface_width
            || self.screen.physical_height != self.surface_height;
        let visible = self.screen.physical_width * self.screen.physical_height != 0;
        if resized && visible {
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
            self.surface_width = self.screen.physical_width;
            self.surface_height = self.screen.physical_height;
        }
    }
}
#[repr(C)]
#[derive(Debug, Default)]
pub struct CUuid([u8; 16]);

impl From<Uuid> for CUuid {
    fn from(value: Uuid) -> Self {
        Self(value.into_bytes())
    }
}

impl From<CUuid> for Uuid {
    fn from(value: CUuid) -> Self {
        Uuid::from_bytes(value.0)
    }
}
