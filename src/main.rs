use wasm_bindgen::prelude::*;
use winit::window::Window;

#[wasm_bindgen(start)]
pub async fn run() {
    // Redirect panics and logs to the browser console
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Info).expect("Couldn't initialize logger");
        }
    }

    // 1. Set up your Winit event loop and window
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let window = Window::new(&event_loop).unwrap();

    // 2. Append canvas to browser DOM on the web
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowExtWebSys;
        let canvas = window.canvas().raw_attribute();

        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(canvas))
                    .ok()
            })
            .expect("Couldn't append canvas to document body.");
    }

    // 3. Initialize wgpu Instance
    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(&window).unwrap();

    // 4. Request Adapter & Device (Must await via wasm-bindgen-futures)
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }).await.unwrap();

    let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default(), None).await.unwrap();

    // Your main render loop goes here...
}