mod app;
mod ble;
mod city_loader;
mod navigation;
mod ride;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::web::WindowAttributesExtWebSys;
use winit::window::{Window, WindowAttributes, WindowId};

use sykla_engine::renderer::Renderer;

use crate::app::AppInner;

/// Outer shell: holds Option<AppInner> behind Rc<RefCell<>>.
/// Before the async Renderer::new() resolves, frames are no-ops.
struct WebApp {
    inner: Rc<RefCell<Option<AppInner>>>,
    window: Option<Arc<Window>>,
    start_time: Instant,
}

impl WebApp {
    fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(None)),
            window: None,
            start_time: Instant::now(),
        }
    }
}

impl ApplicationHandler for WebApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Grab the existing #sykla-canvas from the DOM
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("sykla-canvas"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("Could not find #sykla-canvas element");

        let attrs = WindowAttributes::default()
            .with_canvas(Some(canvas))
            .with_title("Sykla Engine");

        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        self.window = Some(window.clone());
        self.start_time = Instant::now();

        // Async init: spawn_local the renderer creation
        let inner = self.inner.clone();
        spawn_local(async move {
            let renderer = Renderer::new(window.clone()).await;
            let app_inner = AppInner::new(renderer, window);
            *inner.borrow_mut() = Some(app_inner);
            web_sys::console::log_1(&"sykla-engine initialized".into());
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(ref mut app) = *self.inner.borrow_mut() {
                    app.handle_resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(ref mut app) = *self.inner.borrow_mut() {
                    app.handle_key(&event);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(ref mut app) = *self.inner.borrow_mut() {
                    app.handle_cursor_move(position.x as f32, position.y as f32);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                if let Some(ref mut app) = *self.inner.borrow_mut() {
                    app.handle_click();
                }
            }
            WindowEvent::Touch(touch) => {
                if let Some(ref mut app) = *self.inner.borrow_mut() {
                    app.handle_touch(touch);
                }
            }
            WindowEvent::RedrawRequested => {
                let t = self.start_time.elapsed().as_secs_f32();
                if let Some(ref mut app) = *self.inner.borrow_mut() {
                    app.render_frame(t);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

#[wasm_bindgen(start)]
pub fn wasm_main() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&format!("PANIC: {info}").into());
    }));

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = WebApp::new();
    event_loop.run_app(&mut app).unwrap();
}
