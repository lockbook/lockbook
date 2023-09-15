#[macro_export]
macro_rules! tokio_spawn {
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

#[macro_export]
macro_rules! spawn {
    ($block:expr) => {{
        #[cfg(target_arch = "wasm32")]
        {
            // For WASM, wrap the blocking code in an async block
            wasm_bindgen_futures::spawn_local(async move { $block });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || $block);
        }
    }};
}
