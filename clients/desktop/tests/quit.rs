//! AppKit's quit (⌘Q, the app menu's Quit, the Dock's, a logout) must not end the process before
//! the app closes its window the way the close button does: that close is what saves the last
//! edits and writes a try's out file. Nor may the app cancel the quit, which would cancel a logout:
//! once it has closed, AppKit goes on and ends the process. AppKit runs on the main thread, so this
//! runs without the test harness.

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

    // A quit that never goes on.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(10));
        eprintln!("quit: AppKit did not end the process after the app closed its window");
        // SAFETY: ends the process with a failing status.
        unsafe { libc::_exit(1) };
    });

    // AppKit ends a quit with `exit`, which runs this.
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
    impl ApplicationHandler<()> for Quit {
        fn resumed(&mut self, _: &ActiveEventLoop) {
            // From the run loop, as ⌘Q and a logout come: outside winit's event handling.
            let mtm = MainThreadMarker::new().expect("main thread");
            let app = NSApplication::sharedApplication(mtm);
            let nil: *const objc2::runtime::AnyObject = std::ptr::null();
            // SAFETY: `terminate:` takes one object, which may be nil.
            let () = unsafe {
                objc2::msg_send![&app, performSelector: objc2::sel!(terminate:), withObject: nil, afterDelay: 0.0f64]
            };
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
        // The app closes its window here, while AppKit waits on the quit.
        fn user_event(&mut self, _: &ActiveEventLoop, (): ()) {
            CLOSED.store(true, Ordering::SeqCst);
            assert!(lockbook_desktop::shell::macos_window::quit_settled());
        }
    }

    let event_loop = EventLoop::<()>::with_user_event()
        .with_activation_policy(ActivationPolicy::Prohibited)
        .build()
        .expect("event loop");
    let proxy = event_loop.create_proxy();
    lockbook_desktop::shell::macos_window::quit_closes(move || {
        proxy.send_event(()).expect("event loop");
    });
    event_loop.run_app(&mut Quit).expect("event loop");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
