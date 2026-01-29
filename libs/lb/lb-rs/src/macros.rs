#[macro_export]
macro_rules! spawn {
    ($future:expr) => {{
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local($future);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            tokio::spawn($future);
        }
    }};
}
