//! Translate winit's asynchronous file transfers into egui input.
use std::{collections::HashMap, path::PathBuf};
use winit::{
    data_transfer::{DataTransfer, DataTransferId, TypeHint},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, AsyncRequestSerial, DndAction},
};

#[derive(Default)]
pub(crate) struct FileDrops {
    pending: HashMap<DataTransferId, Pending>,
}

struct Pending {
    serial: Option<AsyncRequestSerial>,
    position: Option<winit::dpi::PhysicalPosition<f64>>,
    via_portal: bool,
}

#[cfg(all(target_os = "linux", feature = "wayland"))]
fn portal_type(data: &dyn DataTransfer) -> Option<winit::platform::wayland::MimeType> {
    if !std::path::Path::new("/.flatpak-info").exists() {
        return None;
    }
    data.available_types().into_iter().find_map(|ty| {
        ty.cast_ref::<winit::platform::wayland::MimeType>()
            .filter(|mime| mime.to_string() == "application/vnd.portal.filetransfer")
            .cloned()
    })
}

#[cfg(not(all(target_os = "linux", feature = "wayland")))]
fn portal_type(_data: &dyn DataTransfer) -> Option<TypeHint> {
    None
}

#[cfg(all(target_os = "linux", feature = "wayland"))]
fn retrieve_portal_files(key: &[u8]) -> std::io::Result<Vec<PathBuf>> {
    let key = std::str::from_utf8(key)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?
        .trim_end_matches('\0');
    let files = pollster::block_on(async {
        ashpd::documents::FileTransfer::new()
            .await?
            .retrieve_files(key)
            .await
    })
    .map_err(|err| std::io::Error::other(err.to_string()))?;
    Ok(files.into_iter().map(PathBuf::from).collect())
}

#[cfg(not(all(target_os = "linux", feature = "wayland")))]
fn retrieve_portal_files(_key: &[u8]) -> std::io::Result<Vec<PathBuf>> {
    Err(std::io::ErrorKind::Unsupported.into())
}

impl FileDrops {
    pub fn has_hover(&self) -> bool {
        self.pending
            .values()
            .any(|pending| pending.serial.is_none())
    }

    pub fn on_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        event: &WindowEvent,
    ) -> Option<(Vec<PathBuf>, Option<winit::dpi::PhysicalPosition<f64>>)> {
        match event {
            WindowEvent::DragEntered { id, position } => {
                let supported = self.pending.len() < 8
                    && event_loop.data_transfer(*id).is_ok_and(|data| {
                        data.has_type(&TypeHint::UriList) || portal_type(&*data).is_some()
                    });
                let actions = if supported {
                    &[DndAction::Copy][..]
                } else {
                    &[]
                };
                if event_loop.set_valid_dnd_actions(*id, actions).is_ok() && supported {
                    self.pending.insert(
                        *id,
                        Pending {
                            serial: None,
                            position: *position,
                            via_portal: false,
                        },
                    );
                }
            }
            WindowEvent::DragPosition { id, position, .. } => {
                if let Some(pending) = self.pending.get_mut(id) {
                    pending.position = Some(*position);
                }
            }
            WindowEvent::DragDropped {
                id,
                proposed_action,
            } => {
                if proposed_action.is_some_and(|action| action != DndAction::Copy) {
                    self.pending.remove(id);
                } else if let Some(pending) = self.pending.get_mut(id) {
                    let portal = event_loop
                        .data_transfer(*id)
                        .ok()
                        .and_then(|data| portal_type(&*data));
                    pending.via_portal = portal.is_some();
                    let request = match &portal {
                        Some(portal) => event_loop.fetch_data_transfer(*id, portal),
                        None => event_loop.fetch_data_transfer(*id, &TypeHint::UriList),
                    };
                    match request {
                        Ok(serial) => pending.serial = Some(serial),
                        Err(_) => {
                            self.pending.remove(id);
                        }
                    }
                }
            }
            WindowEvent::DragLeft { id }
                if self.pending.get(id).is_some_and(|p| p.serial.is_none()) =>
            {
                self.pending.remove(id);
            }
            WindowEvent::DataTransferReceived { id, serial, value } => {
                let pending = self.pending.get(id)?;
                if pending.serial != Some(*serial) {
                    return None;
                }
                let paths = if pending.via_portal {
                    value
                        .try_as_bytes()
                        .and_then(|key| retrieve_portal_files(&key))
                } else {
                    value.try_as_uris().and_then(|uris| {
                        parse_uris(&uris).ok_or_else(|| {
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "Invalid local file list",
                            )
                        })
                    })
                };
                match paths {
                    Ok(paths) => {
                        let position = self.pending.remove(id)?.position;
                        if valid_paths(&paths) {
                            return Some((paths, position));
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => {
                        self.pending.remove(id);
                    }
                }
            }
            WindowEvent::Destroyed => self.pending.clear(),
            _ => {}
        }
        None
    }
}

fn valid_paths(paths: &[PathBuf]) -> bool {
    !paths.is_empty()
        && paths.len() <= 256
        && paths
            .iter()
            .all(|p| p.is_absolute() && !p.as_os_str().as_encoded_bytes().contains(&0))
        && paths.iter().map(|p| p.as_os_str().len()).sum::<usize>() <= 1024 * 1024
}

fn parse_uris(uris: &[String]) -> Option<Vec<PathBuf>> {
    if uris.is_empty()
        || uris.len() > 256
        || uris.iter().map(String::len).sum::<usize>() > 1024 * 1024
    {
        return None;
    }
    let paths: Vec<PathBuf> = uris
        .iter()
        .filter(|uri| !uri.is_empty() && !uri.starts_with('#'))
        .map(|uri| {
            let bytes = uri.as_bytes();
            for (index, byte) in bytes.iter().enumerate() {
                if *byte == b'%'
                    && (bytes.get(index + 1).is_none_or(|b| !b.is_ascii_hexdigit())
                        || bytes.get(index + 2).is_none_or(|b| !b.is_ascii_hexdigit()))
                {
                    return None;
                }
            }
            let url = url::Url::parse(uri).ok()?;
            if url.scheme() != "file"
                || url.query().is_some()
                || url.fragment().is_some()
                || !url.username().is_empty()
                || url.password().is_some()
                || url
                    .host_str()
                    .is_some_and(|host| !host.is_empty() && host != "localhost")
            {
                return None;
            }
            url.to_file_path().ok()
        })
        .collect::<Option<_>>()?;
    valid_paths(&paths).then_some(paths)
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_paths_are_local_and_bounded() {
        use std::path::PathBuf;
        let root = std::path::absolute("image with space.png").unwrap();
        assert!(super::valid_paths(core::slice::from_ref(&root)));
        assert!(!super::valid_paths(&[]));
        assert!(!super::valid_paths(&[PathBuf::from("relative.png")]));
        assert!(!super::valid_paths(&vec![root; 257]));
    }

    #[test]
    fn file_uris_reject_remote_credentials_queries_and_invalid_escapes() {
        for uri in [
            "https://example.com/image.png",
            "file://remote/image.png",
            "file://user@localhost/image.png",
            "file:///image.png?query=1",
            "file:///image.png#fragment",
            "file:///image%00.png",
            "file:///image%GG.png",
            "file:///image%.png",
        ] {
            assert!(super::parse_uris(&[uri.into()]).is_none(), "{uri}");
        }
    }

    #[test]
    fn file_uris_preserve_spaces_and_unicode() {
        let path = std::path::absolute("image with space ✓.png").unwrap();
        let uri = url::Url::from_file_path(&path).unwrap();
        assert_eq!(super::parse_uris(&[uri.into()]), Some(vec![path]));
    }
}
