//! Standalone Servo browser host.
//!
//! Servo currently owns a surfman/OpenGL rendering context. Blaze's egui shell
//! uses wgpu, so this host is a separate native window until a zero-copy
//! compositor bridge is implemented and validated.

use std::cell::{Cell, RefCell};
use std::env;
use std::error::Error;
use std::rc::Rc;

use blaze_browser::normalize_address;
use blaze_profile::{PermissionDecision, PermissionKind, SessionTab, SitePermission};
use embedder_traits::EventLoopWaker;
use euclid::Scale;
use servo::{
    NavigationRequest, PermissionRequest,
    InputEvent, Key, KeyState, KeyboardEvent, Modifiers as ServoModifiers,
    MouseButton as ServoMouseButton, MouseButtonAction, MouseButtonEvent,
    MouseLeftViewportEvent, MouseMoveEvent, NamedKey, PermissionFeature,
    RenderingContext,
    Servo, ServoBuilder, WebView, WebViewBuilder, WheelDelta, WheelEvent, WheelMode,
    WindowRenderingContext,
};
use tracing::warn;
use url::Url;
use webrender_api::units::DevicePoint;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowId};

pub fn run() -> Result<(), Box<dyn Error>> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install the Rustls crypto provider");

    let requested = env::args()
        .nth(2)
        .unwrap_or_else(|| "https://example.com/".to_owned());
    let fallback_url = normalize_address(&requested)?;

    let mut restored_tabs = env::var("BLAZE_SESSION_TABS")
        .ok()
        .and_then(|json| serde_json::from_str::<Vec<SessionTab>>(&json).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|tab| {
            let url = normalize_address(&tab.url).ok()?;
            Some(ServoTab {
                title: tab.title,
                url: url.to_string(),
                pinned: tab.pinned,
            })
        })
        .collect::<Vec<_>>();

    if restored_tabs.is_empty() {
        restored_tabs.push(ServoTab {
            title: fallback_url.host_str().unwrap_or("New Tab").to_owned(),
            url: fallback_url.to_string(),
            pinned: false,
        });
    }

    let active_tab = env::var("BLAZE_ACTIVE_TAB")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
        .min(restored_tabs.len() - 1);
    let start_url = Url::parse(&restored_tabs[active_tab].url)?;

    let event_loop = EventLoop::<WakerEvent>::with_user_event().build()?;
    let mut app = App::new(&event_loop, start_url);
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[derive(Clone, Debug)]
struct ServoTab {
    title: String,
    url: String,
    pinned: bool,
}

struct AppState {
    window: Window,
    servo: Servo,
    rendering_context: Rc<WindowRenderingContext>,
    webviews: RefCell<Vec<Option<WebView>>>,
    last_used: RefCell<Vec<u64>>,
    focus_tick: Cell<u64>,
    tabs: RefCell<Vec<ServoTab>>,
    active_tab: Cell<usize>,
    current_url: RefCell<String>,
    cursor_position: Cell<(f64, f64)>,
    modifiers: Cell<ModifiersState>,
    address_editing: Cell<bool>,
    address_buffer: RefCell<String>,
    site_permissions: Vec<SitePermission>,
}

impl AppState {
    fn update_window_title(&self) {
        let tabs = self.tabs.borrow();
        if tabs.is_empty() {
            self.window.set_title("Blaze");
            return;
        }
        let index = self.active_tab.get().min(tabs.len() - 1);
        let tab = &tabs[index];
        let pin = if tab.pinned { "● " } else { "" };
        self.window.set_title(&format!("{pin}{} ({}/{}) — Blaze", tab.title, index + 1, tabs.len()));
    }

    fn active_webview(&self) -> Option<WebView> {
        self.webviews.borrow().get(self.active_tab.get()).and_then(|entry| entry.as_ref().cloned())
    }

    fn tab_index_for_webview(&self, candidate: &WebView) -> Option<usize> {
        self.webviews.borrow().iter().position(|entry| entry.as_ref().is_some_and(|view| view.id() == candidate.id()))
    }

    fn ensure_active_webview(self: &Rc<Self>) {
        let index = self.active_tab.get();
        let url_text = {
            let tabs = self.tabs.borrow();
            let Some(tab) = tabs.get(index) else { return };
            tab.url.clone()
        };
        if self.webviews.borrow().get(index).is_none_or(Option::is_none) {
            match Url::parse(&url_text) {
                Ok(url) => {
                    let view = WebViewBuilder::new(&self.servo, self.rendering_context.clone())
                        .url(url)
                        .hidpi_scale_factor(Scale::new(self.window.scale_factor() as f32))
                        .delegate(self.clone())
                        .build();
                    let mut views = self.webviews.borrow_mut();
                    if views.len() <= index { views.resize_with(index + 1, || None); }
                    views[index] = Some(view);
                }
                Err(error) => { warn!(?error, "Could not create active Blaze WebView"); return; }
            }
        }
        let tick = self.focus_tick.get().saturating_add(1);
        self.focus_tick.set(tick);
        if let Some(value) = self.last_used.borrow_mut().get_mut(index) { *value = tick; }
        {
            let views = self.webviews.borrow();
            for (tab_index, entry) in views.iter().enumerate() {
                if let Some(view) = entry {
                    if tab_index == index { view.show(); view.set_focused(true); }
                    else { view.set_focused(false); view.hide(); }
                }
            }
        }
        *self.current_url.borrow_mut() = url_text;
        self.trim_background_webviews();
        self.update_window_title();
        self.window.request_redraw();
    }

    fn trim_background_webviews(&self) {
        const MAX_LIVE_WEBVIEWS: usize = 2;
        let active = self.active_tab.get();
        let tabs = self.tabs.borrow();
        let mut views = self.webviews.borrow_mut();
        let last_used = self.last_used.borrow();
        let mut live = views.iter().filter(|entry| entry.is_some()).count();
        if live <= MAX_LIVE_WEBVIEWS { return; }
        let mut candidates = views.iter().enumerate().filter_map(|(index, entry)| {
            let tab = tabs.get(index)?;
            (entry.is_some() && index != active && !tab.pinned)
                .then_some((last_used.get(index).copied().unwrap_or(0), index))
        }).collect::<Vec<_>>();
        candidates.sort_by_key(|(tick, _)| *tick);
        for (_, index) in candidates {
            if live <= MAX_LIVE_WEBVIEWS { break; }
            if let Some(view) = views.get_mut(index).and_then(Option::take) {
                view.set_focused(false);
                view.hide();
                live -= 1;
            }
        }
    }

    fn new_tab(self: &Rc<Self>) {
        let url = Url::parse("about:blank").expect("about:blank is valid");
        let index = {
            let mut tabs = self.tabs.borrow_mut();
            tabs.push(ServoTab { title: "New Tab".to_owned(), url: url.to_string(), pinned: false });
            tabs.len() - 1
        };
        self.webviews.borrow_mut().push(None);
        self.last_used.borrow_mut().push(0);
        self.active_tab.set(index);
        self.ensure_active_webview();
    }

    fn close_active_tab(self: &Rc<Self>) {
        let index = self.active_tab.get();
        {
            let mut tabs = self.tabs.borrow_mut();
            if tabs.len() > 1 {
                if tabs.get(index).is_some_and(|tab| tab.pinned) {
                    self.window.set_title("Pinned tab cannot be closed — Blaze");
                    return;
                }
                tabs.remove(index);
                self.webviews.borrow_mut().remove(index);
                self.last_used.borrow_mut().remove(index);
                self.active_tab.set(index.min(tabs.len() - 1));
            } else if let Some(tab) = tabs.first_mut() {
                if tab.pinned { self.window.set_title("Pinned tab cannot be closed — Blaze"); return; }
                tab.title = "New Tab".to_owned();
                tab.url = "about:blank".to_owned();
                if let Some(view) = self.webviews.borrow().first().and_then(Option::as_ref) {
                    view.clear_session_history();
                    view.load(Url::parse("about:blank").expect("about:blank is valid"));
                }
            }
        }
        self.ensure_active_webview();
    }

    fn toggle_active_tab_pin(&self) {
        let mut tabs = self.tabs.borrow_mut();
        let index = self.active_tab.get().min(tabs.len().saturating_sub(1));
        let pinned = if let Some(tab) = tabs.get_mut(index) {
            tab.pinned = !tab.pinned;
            tab.pinned
        } else {
            false
        };
        drop(tabs);
        self.update_window_title();
        if pinned {
            self.window.set_title("Tab pinned — Blaze");
        } else {
            self.window.set_title("Tab unpinned — Blaze");
        }
    }

    fn cycle_tab(self: &Rc<Self>, backwards: bool) {
        let count = self.tabs.borrow().len();
        if count < 2 { return; }
        let current = self.active_tab.get().min(count - 1);
        let next = if backwards { (current + count - 1) % count } else { (current + 1) % count };
        self.active_tab.set(next);
        self.ensure_active_webview();
    }

    fn webview_point(&self) -> servo::WebViewPoint {
        let (x, y) = self.cursor_position.get();
        DevicePoint::new(x as f32, y as f32).into()
    }

    fn begin_address_edit(&self) {
        self.address_editing.set(true);
        *self.address_buffer.borrow_mut() = self.current_url.borrow().clone();
        self.window.set_title(&format!(
            "Address: {}  —  Enter to navigate, Esc to cancel",
            self.address_buffer.borrow()
        ));
    }

    fn append_address_text(&self, text: &str) {
        let mut address = self.address_buffer.borrow_mut();
        address.push_str(text);
        self.window.set_title(&format!(
            "Address: {}  —  Enter to navigate, Esc to cancel",
            *address
        ));
    }

    fn replace_address_text(&self, address: String) {
        *self.address_buffer.borrow_mut() = address;
        self.window.set_title(&format!(
            "Address: {}  —  Enter to navigate, Esc to cancel",
            self.address_buffer.borrow()
        ));
    }

    fn commit_address(self: &Rc<Self>) {
        let requested = self.address_buffer.borrow().clone();
        match normalize_address(&requested) {
            Ok(url) => {
                *self.current_url.borrow_mut() = url.to_string();
                if let Some(tab) = self.tabs.borrow_mut().get_mut(self.active_tab.get()) {
                    tab.url = url.to_string();
                    tab.title = url.host_str().unwrap_or("New Tab").to_owned();
                }
                self.address_editing.set(false);
                self.update_window_title();
                if let Some(webview) = self.active_webview() {
                    webview.load(url);
                }
            }
            Err(error) => {
                self.window.set_title(&format!("Invalid address: {error} — Blaze"));
            }
        }
    }

    fn handle_shortcut(self: &Rc<Self>, event: &winit::event::KeyEvent) -> bool {
        if self.address_editing.get() {
            if event.state != ElementState::Pressed {
                return true;
            }

            let modifiers = self.modifiers.get();
            let control = modifiers.control_key() || modifiers.super_key();
            let alt = modifiers.alt_key();
            match &event.logical_key {
                WinitKey::Named(WinitNamedKey::Enter) => self.commit_address(),
                WinitKey::Named(WinitNamedKey::Escape) => {
                    self.address_editing.set(false);
                    self.update_window_title();
                }
                WinitKey::Named(WinitNamedKey::Backspace) => {
                    self.address_buffer.borrow_mut().pop();
                    self.replace_address_text(self.address_buffer.borrow().clone());
                }
                WinitKey::Character(character)
                    if !control && !alt && !character.chars().any(char::is_control) =>
                {
                    self.append_address_text(character);
                }
                _ => {}
            }
            return true;
        }

        if event.state != ElementState::Pressed {
            return false;
        }

        let modifiers = self.modifiers.get();
        let control = modifiers.control_key() || modifiers.super_key();
        let alt = modifiers.alt_key();
        let lower_character = match &event.logical_key {
            WinitKey::Character(character) => character.to_ascii_lowercase(),
            _ => String::new(),
        };

        if control && lower_character == "l" {
            self.begin_address_edit();
            return true;
        }

        if control && lower_character == "r" {
            if let Some(webview) = self.active_webview() {
                webview.reload();
            }
            return true;
        }

        if control && lower_character == "t" {
            self.new_tab();
            return true;
        }

        if control && lower_character == "w" {
            self.close_active_tab();
            return true;
        }

        if control && lower_character == "p" {
            self.toggle_active_tab_pin();
            return true;
        }

        if control && matches!(&event.logical_key, WinitKey::Named(WinitNamedKey::Tab)) {
            self.cycle_tab(modifiers.shift_key());
            return true;
        }

        if alt {
            match &event.logical_key {
                WinitKey::Named(WinitNamedKey::ArrowLeft) => {
                    if let Some(webview) = self.active_webview() {
                        if webview.can_go_back() {
                            webview.go_back(1);
                        }
                    }
                    return true;
                }
                WinitKey::Named(WinitNamedKey::ArrowRight) => {
                    if let Some(webview) = self.active_webview() {
                        if webview.can_go_forward() {
                            webview.go_forward(1);
                        }
                    }
                    return true;
                }
                _ => {}
            }
        }

        false
    }
}

impl servo::WebViewDelegate for AppState {
    fn notify_new_frame_ready(&self, webview: WebView) {
        // Hidden tabs can keep running tasks, but their new frames do not need
        // to schedule a compositor repaint while another tab is visible.
        if self.tab_index_for_webview(&webview) == Some(self.active_tab.get()) {
            self.window.request_redraw();
        }
    }

    fn notify_page_title_changed(&self, webview: WebView, title: Option<String>) {
        let Some(index) = self.tab_index_for_webview(&webview) else { return };
        if self.address_editing.get() && index == self.active_tab.get() { return; }
        let title = title.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| "New Tab".to_owned());
        if let Some(tab) = self.tabs.borrow_mut().get_mut(index) { tab.title = title; }
        if index == self.active_tab.get() { self.update_window_title(); }
    }

    fn notify_url_changed(&self, webview: WebView, url: Url) {
        let Some(index) = self.tab_index_for_webview(&webview) else { return };
        if let Some(tab) = self.tabs.borrow_mut().get_mut(index) { tab.url = url.to_string(); }
        if index == self.active_tab.get() {
            *self.current_url.borrow_mut() = url.to_string();
            if !self.address_editing.get() { self.update_window_title(); }
        }
    }

    fn request_permission(&self, webview: WebView, request: PermissionRequest) {
        let Some(url) = webview.url() else {
            request.deny();
            return;
        };
        let origin = url.origin().ascii_serialization();
        let kind = match request.feature() {
            PermissionFeature::Camera => PermissionKind::Camera,
            PermissionFeature::Microphone => PermissionKind::Microphone,
            PermissionFeature::Geolocation => PermissionKind::Geolocation,
            PermissionFeature::Notifications => PermissionKind::Notifications,
            // The profile does not currently model these Servo permissions.
            _ => {
                request.deny();
                return;
            }
        };

        let decision = self.site_permissions
            .iter()
            .rev()
            .find(|entry| entry.origin == origin && entry.kind == kind)
            .map(|entry| entry.decision)
            .unwrap_or(PermissionDecision::Ask);

        match decision {
            PermissionDecision::Allow => request.allow(),
            PermissionDecision::Ask | PermissionDecision::Deny => request.deny(),
        }
    }

    fn request_navigation(&self, _webview: WebView, request: NavigationRequest) {
        // Reject file:, javascript:, data:, and custom schemes.
        match (request.url.scheme(), request.url.as_str()) {
            ("http" | "https", _) | ("about", "about:blank") => request.allow(),
            _ => request.deny(),
        }
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
                    .with_title("Blaze Browser")
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
            webviews: RefCell::new(Vec::new()),
            last_used: RefCell::new(Vec::new()),
            focus_tick: Cell::new(0),
            current_url: RefCell::new(initial_url.to_string()),
            tabs: RefCell::new(
                env::var("BLAZE_SESSION_TABS")
                    .ok()
                    .and_then(|json| serde_json::from_str::<Vec<SessionTab>>(&json).ok())
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|tab| {
                        let url = normalize_address(&tab.url).ok()?;
                        Some(ServoTab {
                            title: tab.title,
                            url: url.to_string(),
                            pinned: tab.pinned,
                        })
                    })
                    .collect::<Vec<_>>()
            ),
            active_tab: Cell::new(
                env::var("BLAZE_ACTIVE_TAB")
                    .ok()
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0)
            ),
            cursor_position: Cell::new((0.0, 0.0)),
            modifiers: Cell::new(ModifiersState::empty()),
            address_editing: Cell::new(false),
            address_buffer: RefCell::new(String::new()),
            site_permissions: std::env::var("BLAZE_SITE_PERMISSIONS")
                .ok()
                .and_then(|json| serde_json::from_str::<Vec<SitePermission>>(&json).ok())
                .unwrap_or_default(),
        });

        if state.tabs.borrow().is_empty() {
            state.tabs.borrow_mut().push(ServoTab {
                title: initial_url.host_str().unwrap_or("New Tab").to_owned(),
                url: initial_url.to_string(),
                pinned: false,
            });
        }
        state.active_tab.set(state.active_tab.get().min(state.tabs.borrow().len() - 1));

        let count = state.tabs.borrow().len();
        *state.webviews.borrow_mut() = std::iter::repeat_with(|| None).take(count).collect();
        *state.last_used.borrow_mut() = vec![0; count];
        state.ensure_active_webview();

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
                if let Some(webview) = state.active_webview() {
                    webview.paint();
                    state.rendering_context.present();
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                state.modifiers.set(modifiers.state());
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_position.set((position.x, position.y));
                if let Some(webview) = state.active_webview() {
                    webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(
                        state.webview_point(),
                    )));
                }
            }
            WindowEvent::CursorLeft { .. } => {
                if let Some(webview) = state.active_webview() {
                    webview.notify_input_event(InputEvent::MouseLeftViewport(
                        MouseLeftViewportEvent::default(),
                    ));
                }
            }
            WindowEvent::MouseInput { state: button_state, button, .. } => {
                let servo_button = match button {
                    MouseButton::Left => ServoMouseButton::Primary,
                    MouseButton::Middle => ServoMouseButton::Auxiliary,
                    MouseButton::Right => ServoMouseButton::Secondary,
                    MouseButton::Back => ServoMouseButton::Back,
                    MouseButton::Forward => ServoMouseButton::Forward,
                    MouseButton::Other(value) => ServoMouseButton::Other(value),
                };
                let action = match button_state {
                    ElementState::Pressed => MouseButtonAction::Down,
                    ElementState::Released => MouseButtonAction::Up,
                };
                if let Some(webview) = state.active_webview() {
                    webview.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
                        action,
                        servo_button,
                        state.webview_point(),
                    )));
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !state.handle_shortcut(&event) {
                    if let Some(key) = to_servo_key(&event.logical_key) {
                        let key_state = match event.state {
                            ElementState::Pressed => KeyState::Down,
                            ElementState::Released => KeyState::Up,
                        };
                        let mut keyboard_event =
                            KeyboardEvent::from_state_and_key(key_state, key);
                        keyboard_event.event.modifiers = to_servo_modifiers(state.modifiers.get());
                        if let Some(webview) = state.active_webview() {
                            webview.notify_input_event(InputEvent::Keyboard(keyboard_event));
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(webview) = state.active_webview() {
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
                        state.webview_point(),
                    )));
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(webview) = state.active_webview() {
                    webview.resize(new_size);
                }
                state.window.request_redraw();
            }
            WindowEvent::Focused(focused) => {
                if let Some(webview) = state.active_webview() {
                    webview.set_focused(focused);
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                for webview in state.webviews.borrow().iter().flatten() {
                    webview.set_hidpi_scale_factor(Scale::new(scale_factor as f32));
                }
            }
            _ => {}
        }

        // Servo requires the embedder to pump its event loop after forwarding
        // input or invoking WebView actions.
        state.servo.spin_event_loop();
    }
}

fn to_servo_modifiers(modifiers: ModifiersState) -> ServoModifiers {
    let mut result = ServoModifiers::empty();
    result.set(ServoModifiers::SHIFT, modifiers.shift_key());
    result.set(ServoModifiers::CONTROL, modifiers.control_key());
    result.set(ServoModifiers::ALT, modifiers.alt_key());
    result.set(ServoModifiers::META, modifiers.super_key());
    result
}

fn to_servo_key(key: &WinitKey) -> Option<Key> {
    match key {
        WinitKey::Character(value) => Some(Key::Character(value.to_string())),
        WinitKey::Named(value) => {
            let parsed = format!("{value:?}").parse::<NamedKey>().ok()?;
            Some(Key::Named(parsed))
        }
        _ => None,
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
