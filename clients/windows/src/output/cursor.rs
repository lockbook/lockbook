use egui::CursorIcon;
use windows::{core::*, Win32::Foundation::*, Win32::UI::WindowsAndMessaging::*};

pub fn handle(cursor_icon: CursorIcon) -> bool {
    let windows_cursor = to_windows_cursor(cursor_icon);
    let cursor = unsafe { LoadCursorW(HINSTANCE(0), windows_cursor) }.expect("load cursor icon");
    unsafe { SetCursor(cursor) };

    true
}

// https://github.com/rust-windowing/winit/blob/3eea5054405295d79a9b127a879e7accffa4db53/src/platform_impl/windows/util.rs#L167C1-L192C2
fn to_windows_cursor(cursor_icon: CursorIcon) -> PCWSTR {
    match cursor_icon {
        CursorIcon::Default => IDC_ARROW,
        CursorIcon::Help => IDC_HELP,
        CursorIcon::PointingHand => IDC_HAND,
        CursorIcon::Progress => IDC_APPSTARTING,
        CursorIcon::Wait => IDC_WAIT,
        CursorIcon::Crosshair => IDC_CROSS,
        CursorIcon::Text | CursorIcon::VerticalText => IDC_IBEAM,
        CursorIcon::NotAllowed | CursorIcon::NoDrop => IDC_NO,
        CursorIcon::Grab | CursorIcon::Grabbing | CursorIcon::Move | CursorIcon::AllScroll => {
            IDC_SIZEALL
        }
        CursorIcon::ResizeEast
        | CursorIcon::ResizeWest
        | CursorIcon::ResizeHorizontal
        | CursorIcon::ResizeColumn => IDC_SIZEWE,
        CursorIcon::ResizeNorth
        | CursorIcon::ResizeSouth
        | CursorIcon::ResizeVertical
        | CursorIcon::ResizeRow => IDC_SIZENS,
        CursorIcon::ResizeNorthEast | CursorIcon::ResizeSouthWest | CursorIcon::ResizeNeSw => {
            IDC_SIZENESW
        }
        CursorIcon::ResizeNorthWest | CursorIcon::ResizeSouthEast | CursorIcon::ResizeNwSe => {
            IDC_SIZENWSE
        }

        // use arrow for the missing cases
        CursorIcon::None
        | CursorIcon::ContextMenu
        | CursorIcon::Cell
        | CursorIcon::Alias
        | CursorIcon::Copy
        | CursorIcon::ZoomIn
        | CursorIcon::ZoomOut => IDC_ARROW,
    }
}
