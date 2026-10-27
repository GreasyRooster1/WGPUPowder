use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    max_dim: u32,
}

impl Gpu {
    async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("no adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                // WebGL2 can't meet the default limits, so start low and raise to what the adapter supports
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .expect("no device");

        let size = window.inner_size();
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .unwrap();
        surface.configure(&device, &config);

        let max_dim = device.limits().max_texture_dimension_2d;
        Self { surface, device, queue, config, max_dim }
    }

    fn resize(&mut self, w: u32, h: u32) {
        // Canvases can report 0 or huge hi-DPI sizes, so clamp both ends
        self.config.width = w.clamp(1, self.max_dim);
        self.config.height = h.clamp(1, self.max_dim);
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                // Still usable this frame; reconfigure so the next one matches the surface
                self.surface.configure(&self.device, &self.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return, // skip this frame
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.2, b: 0.3, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            // draw calls go here
        }
        self.queue.submit([encoder.finish()]);
        frame.present();
    }
}

struct App {
    proxy: Option<EventLoopProxy<Gpu>>,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
}

impl ApplicationHandler<Gpu> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() { return; }

        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes().with_title("wgpu");
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            attrs = attrs.with_append(true); // adds the <canvas> to <body>
        }
        let window = Arc::new(el.create_window(attrs).unwrap());
        self.window = Some(window.clone());

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.gpu = Some(pollster::block_on(Gpu::new(window.clone())));
            window.request_redraw();
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(proxy) = self.proxy.take() {
            wasm_bindgen_futures::spawn_local(async move {
                let _ = proxy.send_event(Gpu::new(window).await);
            });
        }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, mut gpu: Gpu) {
        // The web Gpu arrives here once async setup finishes
        if let Some(w) = &self.window {
            let s = w.inner_size();
            gpu.resize(s.width, s.height);
            w.request_redraw();
        }
        self.gpu = Some(gpu);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(gpu) = &mut self.gpu else { return };
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => gpu.resize(s.width, s.height),
            WindowEvent::RedrawRequested => {
                gpu.render();
                self.window.as_ref().unwrap().request_redraw(); // continuous loop
            }
            _ => {}
        }
    }
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        console_log::init_with_level(log::Level::Info).unwrap();
    }
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    let event_loop = EventLoop::<Gpu>::with_user_event().build().unwrap();
    let mut app = App { proxy: Some(event_loop.create_proxy()), window: None, gpu: None };

    #[cfg(not(target_arch = "wasm32"))]
    event_loop.run_app(&mut app).unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app);
    }
}