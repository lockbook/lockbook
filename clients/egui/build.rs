#[cfg(windows)]
fn main() -> std::io::Result<()> {
    winres::WindowsResource::new()
        .set_icon("lockbook.ico")
        .compile()
}

#[cfg(not(windows))]
fn main() {}
