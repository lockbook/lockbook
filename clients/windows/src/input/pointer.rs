use std::collections::HashMap;
use std::mem;
use std::ops::BitAnd;
use std::time::Instant;

use lbeguiapp::WgpuLockbook;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Input::Pointer::*;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Default)]
pub struct PointerManager {
    start_time_by_pointer: HashMap<u32, Instant>,
    start_pos_by_pointer: HashMap<u32, egui::Pos2>,
    button_emitted_by_pointer: HashMap<u32, egui::PointerButton>,
}

impl PointerManager {
    // hugely inspired by winit: https://github.com/rust-windowing/winit/blob/master/src/platform_impl/windows/event_loop.rs#L1829
    // interestingly, the message type doesn't matter; we just need to call GetPointerFrameInfoHistory for relevant information
    pub fn handle(
        &mut self, app: &mut WgpuLockbook, window_handle: HWND, modifiers: egui::Modifiers,
        dpi_scale: f32, pointer_id: u16,
    ) -> bool {
        let pointer_id = pointer_id as _;
        let pointer_infos = {
            let mut entries_count = 0u32;
            let mut pointers_count = 0u32;

            if unsafe {
                GetPointerFrameInfoHistory(
                    pointer_id,
                    &mut entries_count,
                    &mut pointers_count,
                    None,
                )
            }
            .is_err()
            {
                return false;
            }

            let pointer_info_count = (entries_count * pointers_count) as usize;
            let mut pointer_infos = Vec::with_capacity(pointer_info_count);
            if unsafe {
                GetPointerFrameInfoHistory(
                    pointer_id,
                    &mut entries_count,
                    &mut pointers_count,
                    Some(pointer_infos.as_mut_ptr()),
                )
            }
            .is_err()
            {
                return false;
            }
            unsafe { pointer_infos.set_len(pointer_info_count) };

            pointer_infos
        };

        // https://docs.microsoft.com/en-us/windows/desktop/api/winuser/nf-winuser-getpointerframeinfohistory
        // The information retrieved appears in reverse chronological order, with the most recent entry in the first
        // row of the returned array
        for pointer_info in pointer_infos.iter().rev() {
            let mut device_rect = mem::MaybeUninit::uninit();
            let mut display_rect = mem::MaybeUninit::uninit();

            if unsafe {
                GetPointerDeviceRects(
                    pointer_info.sourceDevice,
                    device_rect.as_mut_ptr(),
                    display_rect.as_mut_ptr(),
                )
            }
            .is_err()
            {
                continue;
            }

            let device_rect = unsafe { device_rect.assume_init() };
            let display_rect = unsafe { display_rect.assume_init() };

            // For the most precise himetric to pixel conversion we calculate the ratio between the resolution
            // of the display device (pixel) and the touch device (himetric).
            let himetric_to_pixel_ratio_x = (display_rect.right - display_rect.left) as f64
                / (device_rect.right - device_rect.left) as f64;
            let himetric_to_pixel_ratio_y = (display_rect.bottom - display_rect.top) as f64
                / (device_rect.bottom - device_rect.top) as f64;

            // ptHimetricLocation's origin is 0,0 even on multi-monitor setups.
            // On multi-monitor setups we need to translate the himetric location to the rect of the
            // display device it's attached to.
            let x = display_rect.left as f64
                + pointer_info.ptHimetricLocation.x as f64 * himetric_to_pixel_ratio_x;
            let y = display_rect.top as f64
                + pointer_info.ptHimetricLocation.y as f64 * himetric_to_pixel_ratio_y;

            let mut location = POINT { x: x.floor() as i32, y: y.floor() as i32 };

            if unsafe { ScreenToClient(window_handle, &mut location) }.into() {
            } else {
                continue;
            }

            let normalize_pointer_pressure = |pressure| {
                // https://github.com/rust-windowing/winit/blob/master/src/platform_impl/windows/event_loop.rs#L910C1-L915C2
                pressure as f32 / 1024.0
            };
            let force = match pointer_info.pointerType {
                PT_TOUCH => {
                    let mut touch_info = mem::MaybeUninit::uninit();
                    if unsafe {
                        GetPointerTouchInfo(pointer_info.pointerId, touch_info.as_mut_ptr())
                    }
                    .is_err()
                    {
                        continue;
                    };
                    normalize_pointer_pressure(unsafe { touch_info.assume_init().pressure })
                }
                PT_PEN => {
                    let mut pen_info = mem::MaybeUninit::uninit();
                    if unsafe { GetPointerPenInfo(pointer_info.pointerId, pen_info.as_mut_ptr()) }
                        .is_err()
                    {
                        continue;
                    };
                    normalize_pointer_pressure(unsafe { pen_info.assume_init().pressure })
                }
                _ => 0.0,
            };

            let pos = egui::Pos2 {
                x: (location.x as f64 + x.fract()) as f32 / dpi_scale,
                y: (location.y as f64 + y.fract()) as f32 / dpi_scale,
            };

            // also send pointer events when we receive touch events, similar to ios ffi
            // todo: account for other pointer flags e.g. to distinguish draw from erase
            if has_flag(pointer_info.pointerFlags, POINTER_FLAG_INCONTACT) {
                if let (Some(&start_time), Some(&start_pos), maybe_button) = (
                    self.start_time_by_pointer.get(&pointer_id),
                    self.start_pos_by_pointer.get(&pointer_id),
                    self.button_emitted_by_pointer.get(&pointer_id),
                ) {
                    // pointer has already made contact
                    let long = start_time.elapsed().as_millis() > 400;
                    let moved = (start_pos - pos).length() >= 0.0; // todo: this disables long press right click because otherwise for some reason pen input is delayed so h's look like n's

                    match (maybe_button, moved, long) {
                        (Some(_), _, _) => {
                            // pointer button already determined
                            app.renderer
                                .raw_input
                                .events
                                .push(egui::Event::PointerMoved(pos));
                            app.renderer.raw_input.events.push(egui::Event::Touch {
                                device_id: egui::TouchDeviceId(pointer_id as _),
                                id: pointer_id.into(),
                                phase: egui::TouchPhase::Move,
                                pos,
                                force: Some(force),
                            });
                        }
                        (None, true, _) => {
                            // pointer just moved far enough to be a primary button
                            let button = egui::PointerButton::Primary;
                            self.button_emitted_by_pointer.insert(pointer_id, button);

                            app.renderer
                                .raw_input
                                .events
                                .push(egui::Event::PointerButton {
                                    pos: start_pos,
                                    button,
                                    pressed: true,
                                    modifiers,
                                });
                            app.renderer.raw_input.events.push(egui::Event::Touch {
                                device_id: egui::TouchDeviceId(pointer_id as _),
                                id: pointer_id.into(),
                                phase: egui::TouchPhase::Start,
                                pos,
                                force: Some(force),
                            });

                            // queue moves for next frame
                            app.double_queued_events
                                .push(egui::Event::PointerMoved(pos));
                            app.double_queued_events.push(egui::Event::Touch {
                                device_id: egui::TouchDeviceId(pointer_id as _),
                                id: pointer_id.into(),
                                phase: egui::TouchPhase::Move,
                                pos,
                                force: Some(force),
                            });
                        }
                        (None, false, true) => {
                            // pointer contact just lasted long enough to be a secondary button
                            let button = egui::PointerButton::Secondary;
                            self.button_emitted_by_pointer.insert(pointer_id, button);

                            app.renderer
                                .raw_input
                                .events
                                .push(egui::Event::PointerButton {
                                    pos: start_pos,
                                    button,
                                    pressed: true,
                                    modifiers,
                                });
                            app.renderer.raw_input.events.push(egui::Event::Touch {
                                device_id: egui::TouchDeviceId(pointer_id as _),
                                id: pointer_id.into(),
                                phase: egui::TouchPhase::Start,
                                pos,
                                force: Some(force),
                            });
                        }
                        _ => {
                            // we're still waiting to determine the pointer button
                        }
                    }
                } else {
                    // pointer just made contact
                    self.start_time_by_pointer
                        .insert(pointer_id, Instant::now());
                    self.start_pos_by_pointer.insert(pointer_id, pos);
                }
            } else {
                match (
                    self.start_time_by_pointer.remove(&pointer_id),
                    self.start_pos_by_pointer.remove(&pointer_id),
                    self.button_emitted_by_pointer.remove(&pointer_id),
                ) {
                    (_, _, Some(button)) => {
                        // pointer just left contact after a button was determined
                        // un-press whichever pointer button was pressed
                        app.queued_events.push(egui::Event::PointerButton {
                            pos,
                            button,
                            pressed: false,
                            modifiers,
                        });

                        app.queued_events.push(egui::Event::Touch {
                            device_id: egui::TouchDeviceId(pointer_id as _),
                            id: pointer_id.into(),
                            phase: egui::TouchPhase::End,
                            pos,
                            force: Some(force),
                        });
                    }
                    (Some(_), Some(start_pos), _) => {
                        // pointer just left contact before a button was determined
                        // pointer events emitted in this way are always primary
                        let button = egui::PointerButton::Primary;
                        app.renderer
                            .raw_input
                            .events
                            .push(egui::Event::PointerButton {
                                pos: start_pos,
                                button,
                                pressed: true,
                                modifiers,
                            });
                        app.renderer.raw_input.events.push(egui::Event::Touch {
                            device_id: egui::TouchDeviceId(pointer_id as _),
                            id: pointer_id.into(),
                            phase: egui::TouchPhase::Start,
                            pos,
                            force: Some(force),
                        });

                        // queue releases for next frame
                        let button = egui::PointerButton::Primary;
                        app.double_queued_events.push(egui::Event::PointerButton {
                            pos: start_pos,
                            button,
                            pressed: false,
                            modifiers,
                        });
                        app.double_queued_events.push(egui::Event::Touch {
                            device_id: egui::TouchDeviceId(pointer_id as _),
                            id: pointer_id.into(),
                            phase: egui::TouchPhase::End,
                            pos,
                            force: Some(force),
                        });
                    }
                    _ => {
                        // pointer hasn't made contact and still isn't making contact
                    }
                };
            };
        }

        true
    }
}

// https://github.com/rust-windowing/winit/blob/master/src/platform_impl/windows/util.rs#L50C1-L55C2
fn has_flag<T>(bitset: T, flag: T) -> bool
where
    T: Copy + PartialEq + BitAnd<T, Output = T>,
{
    bitset & flag == flag
}
