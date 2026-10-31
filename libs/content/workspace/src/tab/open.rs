//! A tab's content, built from a bundle of resources rather than the
//! workspace, so a tab can exist without one.

use std::sync::{Arc, RwLock};

use egui::{Context, Id};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::account::Account;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::model::svg;

use crate::file_cache::{FileCache, FilesExt as _};
use crate::link_preview::LinkPreviews;
#[cfg(not(target_family = "wasm"))]
use crate::mind_map::show::MindMap;
use crate::open_guard::OpenGuard;
use crate::resolvers::FileCacheLinkResolver;
use crate::resolvers::image_embed::ImageEmbedResolver;
use crate::search::Search;
use crate::show::DocType;
use crate::space_inspector::show::SpaceInspector;
#[cfg(not(target_family = "wasm"))]
use crate::tab::chat::Chat;
use crate::tab::image_viewer::ImageViewer;
use crate::tab::markdown_editor::{Editor as Markdown, MdConfig, MdResources};
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::svg_editor::SVGEditor;
use crate::tab::{Destination, Fragment, TabContent, TabFailure};
use crate::widgets::image_cache::ImageCache;
use crate::workspace::WsPersistentStore;

/// What a tab's content is built from, for one egui context: textures are
/// allocated through the context, and the image cache and link previews
/// count their changes on it, so a bundle never feeds another context.
#[derive(Clone)]
pub struct TabResources {
    pub ctx: Context,
    pub core: Lb,
    pub files: Arc<RwLock<FileCache>>,
    pub images: ImageCache,
    pub previews: LinkPreviews,
    pub persistence: WsPersistentStore,
    pub account: Account,
    pub guard: Arc<OpenGuard>,
    /// Opens mark the guard's marker: off on a preview's own thread, where a
    /// hang can't freeze the app and a kill mustn't refuse the document.
    pub watched: bool,
}

/// A document as loaded: what it's called, its version, and its bytes.
pub struct Doc {
    pub name: String,
    pub hmac: Option<DocumentHmac>,
    pub bytes: Vec<u8>,
}

/// What a region's remembered height is filed under, if it's remembered:
/// a place is, a moment isn't.
pub fn region_key(dest: &Destination, fragment: Option<&Fragment>, width: f32) -> Option<String> {
    if fragment.is_some_and(|f| !f.persists()) {
        return None;
    }
    let fragment = fragment.map(|f| serde_json::to_string(f).unwrap_or_default());
    let dest = serde_json::to_string(dest).unwrap_or_default();
    Some(format!("{dest}|{}|{}", fragment.unwrap_or_default(), width.round() as i32))
}

impl TabResources {
    /// The height `fragment`'s region had at `width` when last measured
    /// settled, so a slot lays out right before its tab loads.
    pub fn remembered_height(
        &self, dest: &Destination, fragment: Option<&Fragment>, width: f32,
    ) -> Option<f32> {
        self.persistence
            .region_height(&region_key(dest, fragment, width)?)
    }

    pub fn remember_height(
        &self, dest: &Destination, fragment: Option<&Fragment>, width: f32, height: f32,
    ) {
        if let Some(key) = region_key(dest, fragment, width) {
            self.persistence.set_region_height(key, height);
        }
    }

    /// The name the editor is picked by. The cache can trail a create or
    /// rename, so core has the last word.
    pub fn name(&self, id: Uuid) -> Option<String> {
        match self.core.get_file_by_id(id) {
            Ok(file) => Some(file.name),
            Err(_) => self
                .files
                .read()
                .unwrap()
                .get_by_id(id)
                .map(|f| f.name.clone()),
        }
    }

    /// Reads document `id` from core.
    pub fn load(&self, id: Uuid) -> Result<Doc, TabFailure> {
        let name = self.name(id).ok_or_else(|| {
            TabFailure::SimpleMisc(format!("failed to load file: {id} not found"))
        })?;
        let (hmac, bytes) = self.core.read_document_with_hmac(id, false)?;
        Ok(Doc { name, hmac, bytes })
    }

    /// Whether `id` opens without its bytes.
    pub fn is_image(&self, id: Uuid) -> bool {
        let files = self.files.read().unwrap();
        files
            .get_by_id(id)
            .is_some_and(|f| DocType::from_name(&f.name) == DocType::Image)
    }
}

impl TabContent {
    /// Builds the content for `dest`, its widget ids derived from `salt`.
    /// A document other than an image needs `doc`; the open guard refuses
    /// what would hang and marks the open on disk.
    pub fn open(
        dest: &Destination, doc: Option<Doc>, res: &TabResources, salt: Id, read_only: bool,
    ) -> Result<TabContent, TabFailure> {
        let id = match dest {
            Destination::File(id) => *id,
            #[cfg(not(target_family = "wasm"))]
            Destination::MindMap(_) => {
                let contact = res.persistence.get_contact_linked_sites();
                return Ok(TabContent::MindMap(MindMap::new(&res.core, contact)));
            }
            #[cfg(target_family = "wasm")]
            Destination::MindMap(_) => {
                return Err(TabFailure::SimpleMisc("Mind map is not supported here".into()));
            }
            Destination::SpaceInspector(root) => {
                let file = res.files.read().unwrap().get_by_id(*root).cloned();
                let inspector = SpaceInspector::new(&res.core, file, res.ctx.clone());
                return Ok(TabContent::SpaceInspector(inspector));
            }
            Destination::Search(state) => {
                return Ok(TabContent::Search(Search::new(&res.core, &res.ctx, state)));
            }
        };
        if doc.is_none() && res.is_image(id) {
            return Ok(TabContent::Image(ImageViewer::new(id, res.images.clone())));
        }
        let doc = match doc {
            Some(doc) => doc,
            None => res.load(id)?,
        };
        let doc_type = DocType::from_name(&doc.name);
        let ext = doc
            .name
            .split('.')
            .next_back()
            .unwrap_or_default()
            .to_owned();
        let is_text = matches!(
            doc_type,
            DocType::PlainText | DocType::Markdown | DocType::Code | DocType::Unknown
        ) && content_inspector::inspect(&doc.bytes).is_text();
        let text = is_text.then_some(doc_type == DocType::Markdown);
        if let Some(refusal) = res.guard.refusal(id, &doc.bytes, text) {
            tracing::warn!(%id, ?refusal, "refusing to open");
            return Err(TabFailure::Refused(refusal));
        }

        if res.watched {
            res.guard.enter(id);
        }
        let Doc { hmac, bytes, .. } = doc;
        let content = match doc_type {
            DocType::Image => Ok(TabContent::Image(ImageViewer::new(id, res.images.clone()))),
            DocType::PDF => {
                let mut pdf = PdfViewer::new(id, bytes, &res.ctx);
                pdf.salt = salt;
                Ok(TabContent::Pdf(pdf))
            }
            DocType::SVG => Ok(TabContent::Svg(SVGEditor::new(
                &bytes,
                &res.ctx,
                res.core.clone(),
                id,
                hmac,
                &res.persistence,
                read_only,
            ))),
            #[cfg(not(target_family = "wasm"))]
            DocType::Chat => {
                let mut chat = Chat::new(
                    &bytes,
                    id,
                    hmac,
                    res.account.clone(),
                    res.ctx.clone(),
                    Arc::clone(&res.files),
                    &res.core,
                );
                chat.salt = salt;
                Ok(TabContent::Chat(chat))
            }
            _ if is_text => {
                let resolver = FileCacheLinkResolver::new(Arc::clone(&res.files), id)
                    .creating_notes(!read_only);
                let mut md = Markdown::new(
                    &String::from_utf8_lossy(&bytes),
                    id,
                    hmac,
                    MdResources {
                        ctx: res.ctx.clone(),
                        previews: res.previews.clone(),
                        core: res.core.clone(),
                        persistence: res.persistence.clone(),
                        link_resolver: Box::new(resolver),
                        files: Arc::clone(&res.files),
                        embeds: Box::new(ImageEmbedResolver::new(res.images.clone(), id)),
                    },
                    MdConfig { readonly: read_only, ext },
                );
                md.set_salt(salt);
                md.edit.renderer.tab_embeds.res = Some(res.clone());
                md.edit.renderer.tab_embeds.path = vec![id];
                Ok(TabContent::Markdown(md))
            }
            _ => Err(TabFailure::SimpleMisc(format!("Unsupported file extension: {ext}"))),
        };
        if res.watched {
            res.guard.exit(id);
        }
        content
    }

    /// Takes a newer version of the document in place, keeping what's open
    /// and its unsaved edits; false when the content has to be rebuilt.
    pub fn reload(&mut self, hmac: Option<DocumentHmac>, bytes: &[u8]) -> bool {
        match self {
            TabContent::Markdown(md) => {
                md.apply_disk_version(String::from_utf8_lossy(bytes).into(), hmac);
                true
            }
            TabContent::Svg(svg) => {
                svg::buffer::Buffer::reload(
                    &mut svg.buffer.elements,
                    &mut svg.buffer.weak_images,
                    &mut svg.buffer.weak_path_pressures,
                    &mut svg.buffer.weak_viewport_settings,
                    &svg.opened_content,
                    &svg::buffer::Buffer::new(String::from_utf8_lossy(bytes).as_ref()),
                );
                svg.open_file_hmac = hmac;
                true
            }
            #[cfg(not(target_family = "wasm"))]
            TabContent::Chat(chat) => {
                chat.reload(bytes, hmac);
                true
            }
            _ => false,
        }
    }
}
