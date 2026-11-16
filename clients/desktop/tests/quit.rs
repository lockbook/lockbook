//! AppKit's quit (⌘Q, the app menu's Quit, the Dock's) must not end the process before the app
//! closes its window the way the close button does: that close is what saves the last edits and
//! writes a try's out file. AppKit runs on the main thread, so this runs without the test harness.

#[cfg(target_os = "macos")]
fn main() {
    use std::sync::atomic::{AtomicBool, Ordering};

    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
    use winit::window::WindowId;

    static CLOSED: AtomicBool = AtomicBool::new(false);

    // AppKit ends a quit it isn't told to cancel with `exit`, which runs this.
    extern "C" fn closed_first() {
        if !CLOSED.load(Ordering::SeqCst) {
            eprintln!("quit: the process ended without the app closing its window");
            // SAFETY: ends the process with a failing status, from inside `exit`.
            unsafe { libc::_exit(1) };
        }
    }
    // SAFETY: registers a plain function.
    unsafe { libc::atexit(closed_first) };

    struct Quit;
    impl ApplicationHandler for Quit {
        fn resumed(&mut self, _: &ActiveEventLoop) {
            let mtm = MainThreadMarker::new().expect("main thread");
            // SAFETY: on the main thread, as ⌘Q is.
            unsafe { NSApplication::sharedApplication(mtm).terminate(None) };
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }

    let event_loop = EventLoop::builder()
        .with_activation_policy(ActivationPolicy::Prohibited)
        .build()
        .expect("event loop");
    lockbook_desktop::shell::macos_window::quit_closes(|| {
        CLOSED.store(true, Ordering::SeqCst);
        std::process::exit(0);
    });
    event_loop.run_app(&mut Quit).expect("event loop");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
