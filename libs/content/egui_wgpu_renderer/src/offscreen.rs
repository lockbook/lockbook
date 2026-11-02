//! One egui frame rendered into a texture of its own and read back, for
//! previews drawn off screen.

use std::iter;

use egui::TexturesDelta;
use egui::epaint::ClippedPrimitive;
use egui_wgpu::{Renderer, ScreenDescriptor};
use wgpu::{Device, Queue, TextureFormat};

/// The format a renderer for [`render_offscreen`] is built with.
pub const OFFSCREEN_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

/// A frame for [`render_offscreen`]: tessellated, with every texture change
/// its context made since the renderer last saw it.
pub struct OffscreenFrame {
    pub paint_jobs: Vec<ClippedPrimitive>,
    pub textures: TexturesDelta,
    pub size_in_pixels: [u32; 2],
    pub pixels_per_point: f32,
    pub clear: wgpu::Color,
}

/// Renders `frame` through `renderer`, a renderer of the frame's own
/// context, and returns the pixels as tightly packed RGBA rows; None when
/// the GPU didn't hand them back, as for an app it has put in the
/// background. `renderer` must be built with [`OFFSCREEN_FORMAT`] and
/// `sample_count` samples.
pub fn render_offscreen(
    device: &Device, queue: &Queue, renderer: &mut Renderer, frame: OffscreenFrame,
    sample_count: u32,
) -> Option<Vec<u8>> {
    let OffscreenFrame { paint_jobs, mut textures, size_in_pixels, pixels_per_point, clear } =
        frame;
    for (id, deltas) in textures.set.drain() {
        for delta in &deltas {
            renderer.update_texture(device, queue, id, delta);
        }
    }

    let [width, height] = size_in_pixels.map(|n| n.max(1));
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let texture = |label, sample_count, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage,
            view_formats: &[],
        })
    };
    let target = texture(
        "offscreen_target",
        1,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let msaa = texture("offscreen_msaa", sample_count, wgpu::TextureUsages::RENDER_ATTACHMENT);
    let target_view = target.create_view(&Default::default());
    let msaa_view = msaa.create_view(&Default::default());

    let screen = ScreenDescriptor { size_in_pixels: [width, height], pixels_per_point };
    let mut encoder = device.create_command_encoder(&Default::default());
    let prepared = renderer.update_buffers(device, queue, &mut encoder, &paint_jobs, &screen);
    {
        let (view, resolve_target) = match sample_count > 1 {
            true => (&msaa_view, Some(&target_view)),
            false => (&target_view, None),
        };
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("offscreen_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer.render(&mut pass.forget_lifetime(), &paint_jobs, &screen);
    }

    // rows of a copy are padded to the copy alignment
    let row = width * 4;
    let padded =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("offscreen_readback"),
        size: (padded * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        size,
    );
    queue.submit(prepared.into_iter().chain(iter::once(encoder.finish())));

    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |mapped| {
        let _ = tx.send(mapped.is_ok());
    });
    let timeout = Some(std::time::Duration::from_secs(5));
    let polled = device.poll(wgpu::PollType::Wait { submission_index: None, timeout });
    let mapped = polled.is_ok() && rx.try_recv() == Ok(true);
    let mut pixels = Vec::with_capacity((row * height) as usize);
    if mapped {
        if let Ok(range) = slice.get_mapped_range() {
            for line in range.chunks(padded as usize) {
                pixels.extend_from_slice(&line[..row as usize]);
            }
        }
        buffer.unmap();
    }

    for id in textures.free.drain() {
        renderer.free_texture(&id);
    }
    (pixels.len() == (row * height) as usize).then_some(pixels)
}
