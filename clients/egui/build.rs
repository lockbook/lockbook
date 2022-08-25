#[cfg(windows)]
fn main() -> io::Result<()> {
    use std::io;
    use winres::WindowsResource;

    WindowsResource::new().set_icon("lockbook.ico").compile()?;

    Ok(())
}

#[cfg(not(windows))]
fn main() {}
