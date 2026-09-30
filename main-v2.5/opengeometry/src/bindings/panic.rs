#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;

pub(super) fn install_panic_hook() {
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(|info| {
        let _ = js_sys::Reflect::set(
            &js_sys::global(),
            &JsValue::from_str("__opengeometryPanic"),
            &JsValue::from_str(&info.to_string()),
        );
        console_error_panic_hook::hook(info);
    }));
}
