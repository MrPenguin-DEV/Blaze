//! Servo embedded directly into Blaze's existing eframe window.
//!
//! One Servo engine is shared by independent per-tab WebViews. The engine
//! renders offscreen; the resulting frame is uploaded to egui's GPU texture.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use euclid::{Box2D, Point2D, Scale};
use servo::{
    EventLoopWaker, InputEvent, Key, KeyState, KeyboardEvent, MouseButton as ServoMouseButton,
    MouseButtonAction, MouseButtonEvent, MouseMoveEvent, NamedKey, RenderingContext, Servo,
    ServoBuilder, SoftwareRenderingContext, WebView, WebViewBuilder, WebViewDelegate,
    WheelDelta, WheelEvent, WheelMode, NavigationRequest,
};
use tracing::warn;
use url::Url;
use winit::dpi::PhysicalSize;
use webrender_api::units::DevicePoint;

pub struct ServoBrowser {
    servo: Servo,
    rendering: Rc<SoftwareRenderingContext>,
    egui_context: egui::Context,
}

pub struct ServoPage {
    pub webview: WebView,
    needs_paint: Rc<Cell<bool>>,
    active: Rc<Cell<bool>>,
    current_url: Rc<RefCell<String>>,
    title: Rc<RefCell<String>>,
    last_size: Cell<PhysicalSize<u32>>,
}

impl ServoBrowser {
    pub fn new(ctx: egui::Context, size: PhysicalSize<u32>) -> Result<Self, String> {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let size = PhysicalSize::new(size.width.max(1), size.height.max(1));
        let rendering = Rc::new(
            SoftwareRenderingContext::new(size)
                .map_err(|error| format!("Servo offscreen renderer: {error:?}"))?,
        );
        rendering
            .make_current()
            .map_err(|error| format!("Servo renderer activation: {error:?}"))?;

        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(EguiWaker(ctx.clone())))
            .build();

        Ok(Self {
            servo,
            rendering,
            egui_context: ctx,
        })
    }

    pub fn create_page(&self, url: &str, scale: f32) -> Result<ServoPage, String> {
        let parsed = Url::parse(url).map_err(|error| error.to_string())?;
        let needs_paint = Rc::new(Cell::new(true));
        let active = Rc::new(Cell::new(false));
        let current_url = Rc::new(RefCell::new(url.to_owned()));
        let title = Rc::new(RefCell::new(parsed.host_str().unwrap_or("New Tab").to_owned()));
        let delegate = Rc::new(PageDelegate {
            context: self.egui_context.clone(),
            needs_paint: needs_paint.clone(),
            active: active.clone(),
            current_url: current_url.clone(),
            title: title.clone(),
        });

        let webview = WebViewBuilder::new(&self.servo, self.rendering.clone())
            .url(parsed)
            .hidpi_scale_factor(Scale::new(scale))
            .delegate(delegate)
            .build();

        Ok(ServoPage {
            webview,
            needs_paint,
            active,
            current_url,
            title,
            last_size: Cell::new(self.rendering.size()),
        })
    }

    pub fn activate(&self, page: &ServoPage) {
        page.active.set(true);
        page.webview.show();
        page.webview.focus();
        page.needs_paint.set(true);
        self.egui_context.request_repaint();
    }

    pub fn deactivate(&self, page: &ServoPage) {
        page.active.set(false);
        page.webview.blur();
        page.webview.hide();
    }

    pub fn navigate(&self, page: &ServoPage, url: &str) -> Result<(), String> {
        let parsed = Url::parse(url).map_err(|error| error.to_string())?;
        page.webview.load(parsed);
        *page.current_url.borrow_mut() = url.to_owned();
        page.needs_paint.set(true);
        self.servo.spin_event_loop();
        Ok(())
    }

    pub fn spin(&self) {
        self.servo.spin_event_loop();
    }

    pub fn render(&self, page: &ServoPage, size: PhysicalSize<u32>) -> Result<Option<egui::ColorImage>, String> {
        let size = PhysicalSize::new(size.width.max(1), size.height.max(1));
        if page.last_size.get() != size {
            page.webview.resize(size);
            page.last_size.set(size);
            page.needs_paint.set(true);
        }

        self.servo.spin_event_loop();
        if !page.needs_paint.replace(false) {
            return Ok(None);
        }

        self.rendering
            .make_current()
            .map_err(|error| format!("Servo renderer activation: {error:?}"))?;
        page.webview.paint();

        // Read the back buffer before present swaps it.
        let rect = Box2D::new(
            Point2D::new(0, 0),
            Point2D::new(size.width as i32, size.height as i32),
        );
        let image = self.rendering.read_to_image(rect);
        self.rendering.present();

        Ok(image.map(|image| {
            egui::ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                image.as_raw(),
            )
        }))
    }

    pub fn current_url(&self, page: &ServoPage) -> String {
        page.current_url.borrow().clone()
    }

    pub fn title(&self, page: &ServoPage) -> String {
        page.title.borrow().clone()
    }

    pub fn reload(&self, page: &ServoPage) {
        page.webview.reload();
    }

    pub fn back(&self, page: &ServoPage) {
        if page.webview.can_go_back() {
            page.webview.go_back(1);
        }
    }

    pub fn forward(&self, page: &ServoPage) {
        if page.webview.can_go_forward() {
            page.webview.go_forward(1);
        }
    }

    pub fn forward_mouse_move(&self, page: &ServoPage, x: f32, y: f32) {
        page.webview.notify_input_event(InputEvent::MouseMove(
            MouseMoveEvent::new(DevicePoint::new(x, y).into()),
        ));
    }

    pub fn forward_mouse_button(&self, page: &ServoPage, x: f32, y: f32, button: egui::PointerButton, pressed: bool) {
        let button = match button {
            egui::PointerButton::Primary => ServoMouseButton::Primary,
            egui::PointerButton::Secondary => ServoMouseButton::Secondary,
            egui::PointerButton::Middle => ServoMouseButton::Auxiliary,
            egui::PointerButton::Extra1 => ServoMouseButton::Back,
            egui::PointerButton::Extra2 => ServoMouseButton::Forward,
        };
        page.webview.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
            if pressed { MouseButtonAction::Down } else { MouseButtonAction::Up },
            button,
            DevicePoint::new(x, y).into(),
        )));
    }

    pub fn forward_wheel(&self, page: &ServoPage, x: f32, y: f32, dx: f32, dy: f32) {
        page.webview.notify_input_event(InputEvent::Wheel(WheelEvent::new(
            WheelDelta { x: dx as f64, y: dy as f64, z: 0.0, mode: WheelMode::DeltaPixel },
            DevicePoint::new(x, y).into(),
        )));
    }

    pub fn forward_key(&self, page: &ServoPage, key: Key, pressed: bool, modifiers: egui::Modifiers) {
        let mut event = KeyboardEvent::from_state_and_key(
            if pressed { KeyState::Down } else { KeyState::Up },
            key,
        );
        event.event.modifiers.set(servo::Modifiers::SHIFT, modifiers.shift);
        event.event.modifiers.set(servo::Modifiers::CONTROL, modifiers.ctrl);
        event.event.modifiers.set(servo::Modifiers::ALT, modifiers.alt);
        event.event.modifiers.set(servo::Modifiers::META, modifiers.mac_cmd || modifiers.command);
        page.webview.notify_input_event(InputEvent::Keyboard(event));
    }

    pub fn key_from_egui(key: egui::Key) -> Option<Key> {
        if key == egui::Key::Space {
            return None;
        }
        format!("{key:?}")
            .parse::<NamedKey>()
            .ok()
            .filter(|named| *named != NamedKey::Unidentified)
            .map(Key::Named)
    }
}

struct EguiWaker(egui::Context);

impl EventLoopWaker for EguiWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Self(self.0.clone()))
    }

    fn wake(&self) {
        self.0.request_repaint();
    }
}

struct PageDelegate {
    context: egui::Context,
    needs_paint: Rc<Cell<bool>>,
    active: Rc<Cell<bool>>,
    current_url: Rc<RefCell<String>>,
    title: Rc<RefCell<String>>,
}

impl WebViewDelegate for PageDelegate {
    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.needs_paint.set(true);
        if self.active.get() {
            self.context.request_repaint();
        }
    }

    fn notify_url_changed(&self, _webview: WebView, url: Url) {
        *self.current_url.borrow_mut() = url.to_string();
        if self.active.get() {
            self.context.request_repaint();
        }
    }

    fn notify_page_title_changed(&self, _webview: WebView, title: Option<String>) {
        if let Some(title) = title.filter(|title| !title.trim().is_empty()) {
            *self.title.borrow_mut() = title;
        }
        if self.active.get() {
            self.context.request_repaint();
        }
    }

    fn request_navigation(&self, _webview: WebView, request: NavigationRequest) {
        match (request.url.scheme(), request.url.as_str()) {
            ("http" | "https", _) | ("about", "about:blank") => request.allow(),
            _ => request.deny(),
        }
    }

    fn notify_crashed(&self, _webview: WebView, reason: String, backtrace: Option<String>) {
        warn!(%reason, ?backtrace, "Servo webview crashed");
    }
}
