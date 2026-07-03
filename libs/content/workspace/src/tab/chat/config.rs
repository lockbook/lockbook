//! Provider, model, and API-key configuration for the chat tab — the state
//! machine behind the toolbar: resolving the selected provider from the
//! append-only log, loading provider files off the UI thread, and the
//! add-provider flow. Split from the parent purely for reviewability;
//! methods the view or the parent call are `pub(super)`.

use super::*;

impl Chat {
    /// The provider the next turn runs with: this user's per-chat selection
    /// (latest config entry) resolved against the *cached* provider list.
    /// Pure — no file I/O, so it's safe on the UI thread every time config
    /// changes. The list is refreshed by the background loader.
    #[cfg(not(target_family = "wasm"))]
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

    /// Spawn a background load of the provider files and prompt — decrypting
    /// them is too slow for a render frame in debug. No-op while one is in
    /// flight. The result lands in `pump_config` and re-resolves the cache.
    #[cfg(not(target_family = "wasm"))]
    pub(super) fn kick_config_load(&mut self) {
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
    #[cfg(not(target_family = "wasm"))]
    pub(super) fn pump_config(&mut self) {
        if let Some(rx) = &self.config_rx {
            if let Ok(loaded) = rx.try_recv() {
                self.config_rx = None;
                self.providers = loaded.providers;
                self.system_prompt = loaded.system_prompt;
                self.config_loaded = true;
                self.provider = self.resolve_provider();
            }
        }
    }

    /// Record a provider/model pick as a config entry in the transcript —
    /// selection syncs across this user's devices; credentials never do. An
    /// empty `model` means the provider's file default.
    #[cfg(not(target_family = "wasm"))]
    pub(super) fn write_selection(&mut self, provider: String, model: String) {
        self.write_config(lb_rs::model::chat::ChatConfig {
            model: Some(lb_rs::model::chat::ModelSelection { provider, model }),
            ..Default::default()
        });
    }

    /// Record a reasoning-effort pick (`EFFORT_AUTO` to clear).
    #[cfg(not(target_family = "wasm"))]
    pub(super) fn write_effort(&mut self, effort: String) {
        self.write_config(lb_rs::model::chat::ChatConfig {
            effort: Some(effort),
            ..Default::default()
        });
    }

    /// Append a config entry and re-resolve the provider display cache.
    #[cfg(not(target_family = "wasm"))]
    pub(super) fn write_config(&mut self, config: lb_rs::model::chat::ChatConfig) {
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
    #[cfg(not(target_family = "wasm"))]
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

    #[cfg(not(target_family = "wasm"))]
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

    /// Kick a background config reload on first frame and on tab
    /// re-activation (a frame after a gap means the user was away, maybe
    /// editing a provider file). Cheap (a thread spawn); the file reads
    /// happen off-thread and land in `pump_config`.
    #[cfg(not(target_family = "wasm"))]
    pub(super) fn refresh_provider_on_return(&mut self) {
        let now = std::time::Instant::now();
        let away = self
            .last_frame
            .is_none_or(|t| now - t > std::time::Duration::from_millis(300));
        self.last_frame = Some(now);
        if away && self.unshared {
            self.kick_config_load();
        }
    }

    /// Create (or reuse) a provider file from a template, select it for this
    /// chat, and open it in a tab — setup is "click, paste your key". An
    /// existing file is opened untouched, never clobbered.
    #[cfg(not(target_family = "wasm"))]
    pub(super) fn create_provider_file(&mut self, name: &str) {
        use crate::tab::ExtendedOutput as _;
        let Some((_, template)) = TEMPLATES
            .iter()
            .flat_map(|group| group.iter())
            .find(|(n, _)| *n == name)
        else {
            return;
        };
        let path = format!("/.agent/providers/{name}.json");
        let file = match self.core.get_by_path(&path) {
            Ok(file) => file,
            Err(_) => match self.core.create_at_path(&path) {
                Ok(file) => file,
                Err(e) => {
                    tracing::warn!("chat: couldn't create {path}: {e:?}");
                    return;
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
        self.ctx.open_file(file.id, true);
    }

    /// Create (or reuse) the system-prompt file and open it in a tab. Only
    /// a freshly created file gets the preamble template (so customizing
    /// starts from what's actually sent) — an existing file opens untouched
    /// even when empty, because an empty prompt file is a meaningful state:
    /// no system prompt. The date sentence stays out of the template — it
    /// would freeze today into the file.
    #[cfg(not(target_family = "wasm"))]
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
