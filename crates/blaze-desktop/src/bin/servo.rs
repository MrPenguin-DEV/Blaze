//! Standalone Servo browser host.
//!
//! This is deliberately a separate executable while the main Blaze workspace
//! uses egui/wgpu. Servo's current built-in window context is based on
//! surfman/OpenGL; a later renderer bridge can combine it with the workspace UI.

use std::cell::RefCell;
use std::env;
use std::error::Error;
use std::rc::Rc;

use blaze_browser::normalize_address;
use embedder_traits::EventLoopWaker;
use euclid::Scale;
use servo::{
    InputEvent, RenderingContext, Servo, ServoBuilder, WebView, WebViewBuilder, WheelDelta,
    WheelEvent, WheelMode, WindowRenderingContext,
};
use tracing::warn;
use url::Url;
use webrender_api::units::DevicePoint;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowId};

fn main() -> Result<(), Box<dyn Error>> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install the Rustls crypto provider");

    let requested = env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com/".to_owned());
    let start_url = normalize_address(&requested)?;

    let event_loop = EventLoop::<WakerEvent>::with_user_event().build()?;
    let mut app = App::new(&event_loop, start_url);
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct AppState {
    window: Window,
    servo: Servo,
    rendering_context: Rc<WindowRenderingContext>,
    webview: RefCell<Option<WebView>>,
}

impl servo::WebViewDelegate for AppState {
    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.window.request_redraw();
    }
}

enum App {
    Initial(Waker, Url),
    Running(Rc<AppState>),
}

impl App {
    fn new(event_loop: &EventLoop<WakerEvent>, initial_url: Url) -> Self {
        Self::Initial(Waker::new(event_loop), initial_url)
    }
}

impl ApplicationHandler<WakerEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let (waker, initial_url) = match self {
            Self::Initial(waker, url) => (waker.clone(), url.clone()),
            Self::Running(_) => return,
        };

        let display_handle = event_loop
            .display_handle()
            .expect("Failed to get display handle");
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Blaze Browser Engine")
                    .with_inner_size(PhysicalSize::new(1280, 820)),
            )
            .expect("Failed to create Blaze browser window");

        let window_handle = window
            .window_handle()
            .expect("Failed to get window handle");
        let rendering_context = Rc::new(
            WindowRenderingContext::new(display_handle, window_handle, window.inner_size())
                .expect("Could not create Servo rendering context"),
        );
        rendering_context
            .make_current()
            .expect("Could not activate Servo rendering context");

        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(waker))
            .build();
        servo.setup_logging();

        let state = Rc::new(AppState {
            window,
            servo,
            rendering_context,
            webview: RefCell::new(None),
        });

        let webview = WebViewBuilder::new(&state.servo, state.rendering_context.clone())
            .url(initial_url)
            .hidpi_scale_factor(Scale::new(state.window.scale_factor() as f32))
            .delegate(state.clone())
            .build();
        *state.webview.borrow_mut() = Some(webview);
        state.window.request_redraw();

        *self = Self::Running(state);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: WakerEvent) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Self::Running(state) = self else {
            return;
        };

        state.servo.spin_event_loop();

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Some(webview) = state.webview.borrow().as_ref() {
                    webview.paint();
                    state.rendering_context.present();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(webview) = state.webview.borrow().as_ref() {
                    let (delta_x, delta_y, mode) = match delta {
                        MouseScrollDelta::LineDelta(dx, dy) => (
                            (dx * 76.0) as f64,
                            (dy * 76.0) as f64,
                            WheelMode::DeltaLine,
                        ),
                        MouseScrollDelta::PixelDelta(delta) => (
                            delta.x,
                            delta.y,
                            WheelMode::DeltaPixel,
                        ),
                    };

                    webview.notify_input_event(InputEvent::Wheel(WheelEvent::new(
                        WheelDelta {
                            x: delta_x,
                            y: delta_y,
                            z: 0.0,
                            mode,
                        },
                        DevicePoint::default().into(),
                    )));
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(webview) = state.webview.borrow().as_ref() {
                    webview.resize(new_size);
                }
                state.window.request_redraw();
            }
            WindowEvent::Focused(focused) => {
                if let Some(webview) = state.webview.borrow().as_ref() {
                    webview.set_focused(focused);
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(webview) = state.webview.borrow().as_ref() {
                    webview.set_hidpi_scale_factor(Scale::new(scale_factor as f32));
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
struct Waker(EventLoopProxy<WakerEvent>);

#[derive(Debug)]
struct WakerEvent;

impl Waker {
    fn new(event_loop: &EventLoop<WakerEvent>) -> Self {
        Self(event_loop.create_proxy())
    }
}

impl EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }

    fn wake(&self) {
        if let Err(error) = self.0.send_event(WakerEvent) {
            warn!(?error, "Failed to wake Blaze Servo host event loop");
        }
    }
}
