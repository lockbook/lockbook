#[cfg(target_arch = "wasm32")]
use eframe::web_sys;
#[cfg(target_arch = "wasm32")]
use public_site::InitialScreen;
#[cfg(target_arch = "wasm32")]
use public_site::LbWebApp;

#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::JsCast;

/// Pins both demo canvases to the same wgpu backend so the surface formats
/// (and therefore egui's gamma-vs-linear shader path) match across them. With
/// the default `Backends::PRIMARY | GL`, the first runner can land on WebGPU
/// and the second on WebGL fallback, picking different surface formats — the
/// markdown editor then renders #101010 as ~#0A0A0A while the canvas demo
/// renders it correctly. Forcing BROWSER_WEBGPU keeps the byte value identical
/// across both demos.
#[cfg(target_arch = "wasm32")]
fn web_options() -> eframe::WebOptions {
    use eframe::egui_wgpu::WgpuSetup;
    let mut opts = eframe::WebOptions::default();
    if let WgpuSetup::CreateNew(ref mut new) = opts.wgpu_options.wgpu_setup {
        new.instance_descriptor.backends = eframe::wgpu::Backends::BROWSER_WEBGPU;
    }
    opts
}

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
        let editor_demo_el = get_canvas_element("editor-demo");

        let start_result = eframe::WebRunner::new()
            .start(
                editor_demo_el,
                web_options(),
                Box::new(|cc| Ok(Box::new(LbWebApp::new(cc, InitialScreen::Editor)))),
            )
            .await;

        if let Err(e) = start_result {
            panic!("Failed to start editor eframe: {e:?}");
        }

        let canvas_demo_el = get_canvas_element("canvas-demo");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas_demo_el,
                web_options(),
                Box::new(|cc| Ok(Box::new(LbWebApp::new(cc, InitialScreen::Canvas)))),
            )
            .await;

        if let Err(e) = start_result {
            panic!("Failed to start canvas eframe: {e:?}");
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn get_canvas_element(id: &str) -> web_sys::HtmlCanvasElement {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(id))
        .expect(&format!("{} element not found", id))
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect(&format!("{} element is not a HtmlCanvasElement", id))
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}
