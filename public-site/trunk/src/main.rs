#[cfg(target_arch = "wasm32")]
use eframe::web_sys;
#[cfg(target_arch = "wasm32")]
use public_site::InitialScreen;
#[cfg(target_arch = "wasm32")]
use public_site::LbWebApp;

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirect `log` message to `console.log` and friends:

    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let layer = tracing_wasm::WASMLayerConfigBuilder::new()
        .set_console_config(tracing_wasm::ConsoleConfig::ReportWithConsoleColor)
        .build();
    tracing_wasm::set_as_global_default_with_config(layer);

    wasm_bindgen_futures::spawn_local(async {
        let start_result = eframe::WebRunner::new()
            .start(
                "editor-demo",
                Default::default(),
                Box::new(|cc| Ok(Box::new(LbWebApp::new(cc, InitialScreen::Editor)))),
            )
            .await;

        // Remove the loading text and spinner:
        let loading_text = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("editor-loading"));

        if let Some(loading_text) = loading_text {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html("<p> Unexpected error occurred</p>");
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }

        let start_result = eframe::WebRunner::new()
            .start(
                "canvas_demo",
                Default::default(),
                Box::new(|cc| Ok(Box::new(LbWebApp::new(cc, InitialScreen::Canvas)))),
            )
            .await;

        // Remove the loading text and spinner:
        let loading_text = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("canvas-loading"));
        if let Some(loading_text) = loading_text {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}
