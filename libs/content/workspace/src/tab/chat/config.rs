//! Provider, model, and API-key configuration for the chat tab — the state
//! machine behind the toolbar and the connect step: resolving the selected
//! provider from the append-only log, loading and validating provider files
//! off the UI thread, and the key-entry flow. Split from the parent purely
//! for reviewability; methods the view or the parent call are `pub(super)`.

use super::*;

impl Chat {
    /// The provider the next turn runs with: this user's per-chat selection
    /// (latest config entry) resolved against the *cached* provider list.
    /// Pure — no file I/O, so it's safe on the UI thread every time config
    /// changes. The list is refreshed by the background loader.
    pub(super) fn resolve_provider(&self) -> Option<settings::Provider> {
        let selection = latest_selection(&self.entries, &self.account.username);
        let mut provider = settings::resolve(self.providers.clone(), selection.as_ref())?;
        // Per-chat effort overrides the file default, but only where effort
        // applies — a stored pick shouldn't leak onto a non-reasoning model
        // the chat later switched to. `EFFORT_AUTO` clears it.
        if effort_available(&provider) {
            if let Some(eff) = latest_effort(&self.entries, &self.account.username) {
                provider.effort = (eff != EFFORT_AUTO).then_some(eff);
            }
        }
        Some(provider)
    }

    /// The first-run stage, or `None` when the agent is ready to chat.
    /// Reads the cached provider + the `/models` result (our validation
    /// signal): a listing means reachable and authenticated.
    pub(super) fn onboard_stage(&self) -> Option<Onboard> {
        let Some(p) = &self.provider else { return Some(Onboard::Choose) };
        let key = (p.name.clone(), p.base_url.clone());
        let err = self
            .models_err
            .as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, e)| e);
        // A real key the server rejected — the composer's "fix key" button.
        // (An unconfigured provider never resolves here: it's filtered at
        // load, so it shows the roster, exactly like a missing file.)
        if err.is_some_and(|e| is_auth_error(e)) {
            return Some(Onboard::NeedKey { name: p.name.clone(), label: p.label() });
        }
        if err.is_some() {
            let local = p.base_url.contains("localhost") || p.base_url.contains("127.0.0.1");
            return Some(Onboard::Unreachable { label: p.label(), local });
        }
        if self.models.as_ref().is_none_or(|(k, _)| *k != key) {
            return Some(Onboard::Connecting(p.label()));
        }
        if p.model.trim().is_empty() {
            return Some(Onboard::PickModel(p.label()));
        }
        None
    }

    /// Spawn a background load of the provider files and prompt — decrypting
    /// them is too slow for a render frame in debug. No-op while one is in
    /// flight. The result lands in `pump_config` and re-resolves the cache.
    /// `pub(crate)` so the workspace can re-kick it when this tab is activated.
    pub(crate) fn kick_config_load(&mut self) {
        if self.config_rx.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.config_rx = Some(rx);
        let core = self.core.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let loaded = LoadedConfig {
                providers: settings::load(&core),
                system_prompt: settings::system_prompt(&core),
            };
            let _ = tx.send(loaded);
            ctx.request_repaint();
        });
    }

    /// Fold a completed background config load into the caches.
    pub(super) fn pump_config(&mut self) {
        if let Some(rx) = &self.config_rx {
            if let Ok(loaded) = rx.try_recv() {
                self.config_rx = None;
                // Reloads fire on every tab return, so most land identical
                // config; only when the resolved provider actually changed
                // (a pasted key, an edited url) is the models cache stale —
                // clearing it every time would re-validate on every return
                // and flicker the picker.
                let before = self.provider.clone();
                self.providers = loaded.providers;
                self.system_prompt = loaded.system_prompt;
                self.config_loaded = true;
                self.provider = self.resolve_provider();
                if self.provider != before {
                    self.models = None;
                    self.models_err = None;
                    self.models_attempt = None;
                }
            }
        }
    }

    /// Record a provider/model pick as a config entry in the transcript —
    /// selection syncs across this user's devices; credentials never do. An
    /// empty `model` means the provider's file default.
    pub(super) fn write_selection(&mut self, provider: String, model: String) {
        self.write_config(lb_rs::model::chat::ChatConfig {
            model: Some(lb_rs::model::chat::ModelSelection { provider, model }),
            ..Default::default()
        });
    }

    /// Record a reasoning-effort pick (`EFFORT_AUTO` to clear).
    pub(super) fn write_effort(&mut self, effort: String) {
        self.write_config(lb_rs::model::chat::ChatConfig {
            effort: Some(effort),
            ..Default::default()
        });
    }

    /// Append a config entry and re-resolve the provider display cache.
    fn write_config(&mut self, config: lb_rs::model::chat::ChatConfig) {
        let msg =
            Message::config_entry(self.account.username.clone(), Utc::now().timestamp(), config);
        self.entries.push(Entry::new(
            msg,
            &self.ctx,
            Arc::clone(&self.composer.renderer.files),
            self.id,
        ));
        self.seq += 1;
        self.provider = self.resolve_provider();
    }

    /// Kick a background `/models` fetch for the picker, unless a matching
    /// success is cached, a fetch is in flight, or a recent failure is
    /// still cooling down.
    pub(super) fn fetch_models(&mut self, provider: &settings::Provider) {
        let key: ModelsKey = (provider.name.clone(), provider.base_url.clone());
        let cached = self.models.as_ref().is_some_and(|(k, _)| *k == key);
        let cooling = self
            .models_attempt
            .as_ref()
            .is_some_and(|(k, at)| *k == key && at.elapsed() < MODELS_RETRY);
        if cached || cooling || self.models_rx.is_some() {
            return;
        }
        self.models_attempt = Some((key.clone(), std::time::Instant::now()));
        let (tx, rx) = std::sync::mpsc::channel();
        self.models_rx = Some(rx);
        let ctx = self.ctx.clone();
        let provider = provider.clone();
        std::thread::spawn(move || {
            let result = match provider.kind.as_str() {
                "anthropic" => anthropic::list_models_blocking(&provider),
                _ => openai::list_models_blocking(&provider),
            };
            let _ = tx.send((key, result));
            ctx.request_repaint();
        });
    }

    pub(super) fn pump_models(&mut self) {
        if let Some(rx) = &self.models_rx {
            if let Ok((key, result)) = rx.try_recv() {
                self.models_rx = None;
                match result {
                    Ok(list) => {
                        self.models = Some((key, list));
                        self.models_err = None;
                    }
                    Err(e) => self.models_err = Some((key, e)),
                }
            }
        }
    }

    /// Kick the one-time background config load on the chat's first frame.
    /// Reloads that pick up a provider file edited in another tab are driven
    /// by the workspace on tab activation (`mark_current_tab_changed`), not a
    /// frame-gap heuristic — that fired on every idle repaint, re-reading every
    /// provider file every few seconds.
    pub(super) fn kick_initial_config_load(&mut self) {
        if self.last_frame.is_none() && self.unshared {
            self.kick_config_load();
        }
        self.last_frame = Some(std::time::Instant::now());
    }

    /// Create (or reuse) a provider file from a template and select it for
    /// this chat. Returns the file id. An existing file is never clobbered.
    fn ensure_provider_file(&mut self, name: &str) -> Option<Uuid> {
        let (_, template) = TEMPLATES
            .iter()
            .flat_map(|group| group.iter())
            .find(|(n, _)| *n == name)?;
        let path = format!("/.agent/providers/{name}.json");
        let file = match self.core.get_by_path(&path) {
            Ok(file) => file,
            Err(_) => match self.core.create_at_path(&path) {
                Ok(file) => file,
                Err(e) => {
                    tracing::warn!("chat: couldn't create {path}: {e:?}");
                    return None;
                }
            },
        };
        // Fill the template only when there's nothing to clobber.
        if self
            .core
            .read_document(file.id, false)
            .is_ok_and(|b| b.is_empty())
        {
            let _ = self.core.write_document(file.id, template.as_bytes());
        }
        self.write_selection(name.to_string(), String::new());
        // The new file isn't in the cached list yet; reload so it resolves
        // and appears in the picker.
        self.kick_config_load();
        Some(file.id)
    }

    /// Add a provider: create its file and select it, then either prompt for
    /// the key (the masked form), open the file (`custom`, which needs more
    /// than a key), or nothing more (`ollama`, keyless — the "pick a model"
    /// step guides). Picking a provider whose file already holds a real key
    /// (it was configured before) just selects it — no connect step, so the
    /// chooser doubles as a provider switcher without re-prompting.
    pub(super) fn begin_add_provider(&mut self, name: &'static str) {
        let Some(file_id) = self.ensure_provider_file(name) else { return };
        match name {
            "ollama" => {}
            "custom" => {
                // Hand-editing is the flow for a blank custom file; the
                // "add key" button still reaches the masked field otherwise.
                self.open_provider_file(file_id);
            }
            _ if self.file_has_real_key(file_id) => {}
            _ => {
                let label = TEMPLATES
                    .iter()
                    .flat_map(|g| g.iter())
                    .find(|(n, _)| *n == name)
                    .map(|(n, j)| template_label(n, j))
                    .unwrap_or_else(|| name.to_string());
                self.open_key_entry(name, label, file_id);
            }
        }
    }

    /// Whether a provider file's `api_key` is a real key — not empty and not
    /// the template placeholder. Read fresh from the file (the cached provider
    /// list can lag a just-written key by a reload).
    fn file_has_real_key(&self, file_id: Uuid) -> bool {
        self.core
            .read_document(file_id, false)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|j| j.get("api_key").and_then(|v| v.as_str().map(String::from)))
            .is_some_and(|k| !k.trim().is_empty() && k != KEY_PLACEHOLDER)
    }

    /// Open the connect step for `name`'s provider file.
    pub(super) fn open_key_entry(&mut self, name: &str, label: String, file_id: Uuid) {
        self.key_field.set_text("");
        // Until the connect card lays out and records the real rect, there is
        // no field position — a stale rect from a prior connect step would let
        // the focus one-shot fire against the wrong spot.
        self.key_field_rect = Rect::NOTHING;
        // From the cached list (by name — `self.provider` can briefly resolve
        // elsewhere). A just-created file may not be in the cache yet: None,
        // which no real key compares equal to.
        let seen_key = self
            .providers
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.api_key.clone());
        self.key_entry = Some(KeyEntry {
            label,
            name: name.to_string(),
            file_id,
            needs_focus: true,
            was_focused: false,
            seen_key,
            connecting: false,
            attempted: false,
        });
    }

    /// Write the entered key into the provider file (preserving its other
    /// fields) and reload so it re-validates. Keeps the entry open in a
    /// `connecting` state; `poll_key_connection` resolves it to done (valid)
    /// or back to an editable "rejected" state, so submit never re-pops.
    pub(super) fn connect_provider_key(&mut self) {
        let Some(entry) = self.key_entry.as_ref() else { return };
        let key = self
            .key_field
            .renderer
            .buffer
            .current
            .text
            .trim()
            .to_string();
        if key.is_empty() {
            return;
        }
        let file_id = entry.file_id;
        if let Some(entry) = self.key_entry.as_mut() {
            entry.connecting = true;
            entry.attempted = true;
            // The form now accounts for this key — only a *different* real
            // key landing from outside closes it.
            entry.seen_key = Some(key.clone());
        }
        let Ok(bytes) = self.core.read_document(file_id, false) else {
            // The provider file is gone (deleted while the step was open) —
            // nowhere to put the key. Close; resolution falls to the chooser.
            self.key_entry = None;
            return;
        };
        if let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            json["api_key"] = serde_json::Value::String(key.clone());
            if let Ok(out) = serde_json::to_vec_pretty(&json) {
                let _ = self.core.write_document(file_id, &out);
            }
        }
        // Apply the key to the in-memory provider now, before the reload lands.
        // Otherwise the per-frame `fetch_models` would fire with the stale
        // placeholder key and its 401 — keyed only by name+base_url — would be
        // read back as a rejection of the key we just pasted.
        let name = self.key_entry.as_ref().map(|e| e.name.clone());
        if let Some(p) = self
            .provider
            .as_mut()
            .filter(|p| Some(&p.name) == name.as_ref())
        {
            p.api_key = Some(key);
        }
        // Drop the stale pre-submit validation so `poll_key_connection` waits
        // for the fresh fetch rather than reading the old error as a rejection.
        self.models = None;
        self.models_err = None;
        self.models_attempt = None;
        self.kick_config_load();
    }

    /// Keep the key entry consistent with the world: close it when its
    /// provider file has been deleted, clear it when the provider now reaches
    /// `/models`, or drop a `connecting` submit back to an editable state on
    /// failure. Anything else = still connecting.
    pub(super) fn poll_key_connection(&mut self) {
        let Some(entry) = self.key_entry.as_ref() else { return };
        // A reload can land a real key under the open form (synced from
        // another device, or hand-edited in another tab): the form's job was
        // done elsewhere — close it. `seen_key` keeps this from closing over
        // a key the form itself is showing status for; a `connecting` submit
        // resolves through its own paths below.
        if !entry.connecting {
            let current = self
                .providers
                .iter()
                .find(|p| p.name == entry.name)
                .and_then(|p| p.api_key.as_deref());
            if let Some(k) = current {
                if !k.trim().is_empty()
                    && k != KEY_PLACEHOLDER
                    && entry.seen_key.as_deref() != Some(k)
                {
                    self.key_entry = None;
                    return;
                }
            }
        }
        if !entry.connecting {
            return;
        }
        // The provider file vanished mid-validation (deleted from the file
        // tree, or a sync removal) — without this the spinner waits forever
        // for a listing that can never arrive. Gated on no reload in flight,
        // so a stale provider list can't be mistaken for a deletion.
        if self.config_loaded
            && self.config_rx.is_none()
            && !self.providers.iter().any(|p| p.name == entry.name)
        {
            self.key_entry = None;
            return;
        }
        let Some(p) = self.provider.as_ref().filter(|p| p.name == entry.name) else { return };
        let key = (p.name.clone(), p.base_url.clone());
        // The connecting state drives its own validation fetch: the toolbar's
        // per-frame `fetch_models` doesn't run while the connect step hides
        // the composer, and the submit cleared the caches it would read.
        let live = p.clone();
        self.fetch_models(&live);
        if self.models.as_ref().is_some_and(|(k, _)| *k == key) {
            self.key_entry = None;
        } else if self.models_err.as_ref().is_some_and(|(k, _)| *k == key) {
            // Any failure — bad key or unreachable server — stops the spinner
            // and returns to an editable state; the step's status line says
            // which, so a dropped connection doesn't read as a bad key.
            if let Some(entry) = self.key_entry.as_mut() {
                entry.connecting = false;
                entry.needs_focus = true;
            }
        }
    }

    /// Open a provider file with the selection over its key placeholder, so
    /// the user can type/paste straight over it. Falls back to a plain open.
    pub(super) fn open_provider_file(&self, file_id: Uuid) {
        use crate::tab::ExtendedOutput as _;
        let range = self
            .core
            .read_document(file_id, false)
            .ok()
            .and_then(|bytes| {
                String::from_utf8_lossy(&bytes)
                    .find(KEY_PLACEHOLDER)
                    .map(|p| p..p + KEY_PLACEHOLDER.len())
            });
        match range {
            Some(r) => self.ctx.open_file_at_range(file_id, r, true),
            None => self.ctx.open_file(file_id, true),
        }
    }

    /// Create (or reuse) the system-prompt file and open it in a tab. Only
    /// a freshly created file gets the preamble template (so customizing
    /// starts from what's actually sent) — an existing file opens untouched
    /// even when empty, because an empty prompt file is a meaningful state:
    /// no system prompt. The date sentence stays out of the template — it
    /// would freeze today into the file.
    pub(super) fn create_prompt_file(&mut self) {
        use crate::tab::ExtendedOutput as _;
        let path = settings::PROMPT_PATH;
        let file = match self.core.get_by_path(path) {
            Ok(file) => file,
            Err(_) => match self.core.create_at_path(path) {
                Ok(file) => {
                    let template = format!("{}\n", harness::PREAMBLE);
                    let _ = self.core.write_document(file.id, template.as_bytes());
                    file
                }
                Err(e) => {
                    tracing::warn!("chat: couldn't create {path}: {e:?}");
                    return;
                }
            },
        };
        self.ctx.open_file(file.id, true);
    }
}
