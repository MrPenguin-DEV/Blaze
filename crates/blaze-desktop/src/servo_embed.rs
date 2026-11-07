//! Servo embedded directly into Blaze's existing eframe OpenGL context.
//!
//! Servo renders into an FBO texture owned by the same GL context that eframe
//! uses. The texture is registered with egui as a native GL texture, avoiding
//! a GPU-to-CPU readback and a second upload for every browser frame.

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString, c_void};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;

use eframe::{egui, glow};
use euclid::Scale;
use gleam::gl::{self, Gl};
use servo::{
    EventLoopWaker, InputEvent, Key, KeyState, KeyboardEvent, MouseButton as ServoMouseButton,
    MouseButtonAction, MouseButtonEvent, MouseMoveEvent, NamedKey, NavigationRequest,
    RgbaImage, RenderingContext, Servo, ServoBuilder, WebView, WebViewBuilder, WebViewDelegate,
    WheelDelta, WheelEvent, WheelMode,
};
use tracing::warn;
use url::Url;
use winit::dpi::PhysicalSize;
use webrender_api::units::{DeviceIntPoint, DeviceIntRect, DevicePoint};

pub struct ServoBrowser {
    // One Servo engine is shared by every tab; each WebView owns its render target.
    servo: Servo,
    glow_gl: Arc<glow::Context>,
    get_proc_address: Arc<dyn Fn(&CStr) -> *const c_void + Send + Sync>,
    initial_size: PhysicalSize<u32>,
    egui_context: egui::Context,
}

pub struct ServoPage {
    pub webview: WebView,
    needs_paint: Rc<Cell<bool>>,
    active: Rc<Cell<bool>>,
    current_url: Rc<RefCell<String>>,
    title: Rc<RefCell<String>>,
    last_size: Cell<PhysicalSize<u32>>,
    rendering: Rc<EguiGlRenderingContext>,
}

struct EguiGlRenderingContext {
    gleam_gl: Rc<dyn Gl>,
    glow_gl: Arc<glow::Context>,
    size: Cell<PhysicalSize<u32>>,
    framebuffer_id: u32,
    renderbuffer_id: u32,
    texture_id: u32,
    previous_framebuffer: Cell<u32>,
}

impl EguiGlRenderingContext {
    #[expect(unsafe_code)]
    fn load_gleam_api(
        get_proc_address: Arc<dyn Fn(&CStr) -> *const c_void + Send + Sync>,
    ) -> Rc<dyn Gl> {
        let api = unsafe {
            gl::GlFns::load_with(|name| {
                let name = CString::new(name).expect("OpenGL symbol names cannot contain NUL");
                get_proc_address(&name)
            })
        };
        api
    }

    fn new(
        glow_gl: Arc<glow::Context>,
        get_proc_address: Arc<dyn Fn(&CStr) -> *const c_void + Send + Sync>,
        size: PhysicalSize<u32>,
    ) -> Result<Self, String> {
        let size = PhysicalSize::new(size.width.max(1), size.height.max(1));
        let gleam_gl = Self::load_gleam_api(get_proc_address);

        let framebuffer_ids = gleam_gl.gen_framebuffers(1);
        let texture_ids = gleam_gl.gen_textures(1);
        let renderbuffer_ids = gleam_gl.gen_renderbuffers(1);
        let (Some(&framebuffer_id), Some(&texture_id), Some(&renderbuffer_id)) =
            (framebuffer_ids.first(), texture_ids.first(), renderbuffer_ids.first())
        else {
            return Err("OpenGL did not allocate the browser framebuffer.".into());
        };

        gleam_gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer_id);
        gleam_gl.bind_texture(gl::TEXTURE_2D, texture_id);
        gleam_gl.tex_image_2d(
            gl::TEXTURE_2D,
            0,
            gl::RGBA as gl::GLint,
            size.width as gl::GLsizei,
            size.height as gl::GLsizei,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            None,
        );
        gleam_gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as gl::GLint);
        gleam_gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as gl::GLint);
        gleam_gl.framebuffer_texture_2d(
            gl::FRAMEBUFFER,
            gl::COLOR_ATTACHMENT0,
            gl::TEXTURE_2D,
            texture_id,
            0,
        );

        gleam_gl.bind_renderbuffer(gl::RENDERBUFFER, renderbuffer_id);
        gleam_gl.renderbuffer_storage(
            gl::RENDERBUFFER,
            gl::DEPTH_COMPONENT24,
            size.width as gl::GLsizei,
            size.height as gl::GLsizei,
        );
        gleam_gl.framebuffer_renderbuffer(
            gl::FRAMEBUFFER,
            gl::DEPTH_ATTACHMENT,
            gl::RENDERBUFFER,
            renderbuffer_id,
        );
        gleam_gl.bind_texture(gl::TEXTURE_2D, 0);

        let status = gleam_gl.check_frame_buffer_status(gl::FRAMEBUFFER);
        gleam_gl.bind_framebuffer(gl::FRAMEBUFFER, 0);
        if status != gl::FRAMEBUFFER_COMPLETE {
            gleam_gl.delete_textures(&[texture_id]);
            gleam_gl.delete_renderbuffers(&[renderbuffer_id]);
            gleam_gl.delete_framebuffers(&[framebuffer_id]);
            return Err(format!("Browser OpenGL framebuffer is incomplete (0x{status:x})."));
        }

        Ok(Self {
            gleam_gl,
            glow_gl,
            size: Cell::new(size),
            framebuffer_id,
            renderbuffer_id,
            texture_id,
            previous_framebuffer: Cell::new(0),
        })
    }

    fn native_texture(&self) -> glow::NativeTexture {
        glow::NativeTexture(
            NonZeroU32::new(self.texture_id)
                .expect("OpenGL texture names are non-zero"),
        )
    }

}

impl Drop for EguiGlRenderingContext {
    fn drop(&mut self) {
        self.gleam_gl.bind_framebuffer(gl::FRAMEBUFFER, 0);
        self.gleam_gl.delete_textures(&[self.texture_id]);
        self.gleam_gl.delete_renderbuffers(&[self.renderbuffer_id]);
        self.gleam_gl.delete_framebuffers(&[self.framebuffer_id]);
    }
}

impl RenderingContext for EguiGlRenderingContext {
    fn prepare_for_rendering(&self) {
        // Servo and egui share one GL context. Preserve eframe's active target
        // so Servo never leaves its offscreen browser FBO bound for UI painting.
        let mut previous = [0];
        #[expect(unsafe_code)]
        unsafe {
            self.gleam_gl.get_integer_v(gl::FRAMEBUFFER_BINDING, &mut previous);
        }
        self.previous_framebuffer.set(previous[0].max(0) as u32);
        self.gleam_gl.bind_framebuffer(gl::FRAMEBUFFER, self.framebuffer_id);
    }

    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        // Keep the eframe render target intact while copying pixels out of Servo's FBO.
        let width = source_rectangle.width() as usize;
        let height = source_rectangle.height() as usize;
        let stride = width.checked_mul(4)?;
        let mut previous = [0];
        #[expect(unsafe_code)]
        unsafe {
            self.gleam_gl.get_integer_v(gl::FRAMEBUFFER_BINDING, &mut previous);
        }
        self.gleam_gl.bind_framebuffer(gl::FRAMEBUFFER, self.framebuffer_id);
        let mut pixels = self.gleam_gl.read_pixels(
            source_rectangle.min.x,
            source_rectangle.min.y,
            width as i32,
            height as i32,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
        );

        self.gleam_gl.bind_framebuffer(gl::FRAMEBUFFER, previous[0].max(0) as u32);
        let original = pixels.clone();
        for y in 0..height {
            let destination = y * stride;
            let source = (height - y - 1) * stride;
            pixels[destination..destination + stride]
                .copy_from_slice(&original[source..source + stride]);
        }

        RgbaImage::from_raw(width as u32, height as u32, pixels)
    }

    fn size(&self) -> PhysicalSize<u32> {
        self.size.get()
    }

    fn resize(&self, size: PhysicalSize<u32>) {
        let size = PhysicalSize::new(size.width.max(1), size.height.max(1));
        if self.size.get() == size {
            return;
        }

        self.gleam_gl.bind_texture(gl::TEXTURE_2D, self.texture_id);
        self.gleam_gl.tex_image_2d(
            gl::TEXTURE_2D,
            0,
            gl::RGBA as gl::GLint,
            size.width as gl::GLsizei,
            size.height as gl::GLsizei,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            None,
        );
        self.gleam_gl.bind_texture(gl::TEXTURE_2D, 0);
        self.gleam_gl.bind_renderbuffer(gl::RENDERBUFFER, self.renderbuffer_id);
        self.gleam_gl.renderbuffer_storage(
            gl::RENDERBUFFER,
            gl::DEPTH_COMPONENT24,
            size.width as gl::GLsizei,
            size.height as gl::GLsizei,
        );
        self.gleam_gl.bind_renderbuffer(gl::RENDERBUFFER, 0);
        self.size.set(size);
    }

    fn present(&self) {
        // Restore eframe's draw target before egui paints its widgets. Leaving
        // the browser FBO bound causes UI feedback, trails, flicker and
        // apparent duplicate/transparent window contents.
        self.gleam_gl.bind_framebuffer(
            gl::FRAMEBUFFER,
            self.previous_framebuffer.get(),
        );
    }

    fn make_current(&self) -> Result<(), surfman::Error> {
        // eframe owns and has already made the shared GL context current.
        Ok(())
    }

    fn gleam_gl_api(&self) -> Rc<dyn Gl> {
        self.gleam_gl.clone()
    }

    fn glow_gl_api(&self) -> Arc<glow::Context> {
        self.glow_gl.clone()
    }
}

impl ServoBrowser {
    pub fn new(
        ctx: egui::Context,
        glow_gl: Arc<glow::Context>,
        get_proc_address: Arc<dyn Fn(&CStr) -> *const c_void + Send + Sync>,
        size: PhysicalSize<u32>,
    ) -> Result<Self, String> {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let size = PhysicalSize::new(size.width.max(1), size.height.max(1));
        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(EguiWaker(ctx.clone())))
            .build();

        Ok(Self {
            servo,
            glow_gl,
            get_proc_address,
            initial_size: size,
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

        let rendering = Rc::new(EguiGlRenderingContext::new(
            self.glow_gl.clone(),
            self.get_proc_address.clone(),
            self.initial_size,
        )?);
        let webview = WebViewBuilder::new(&self.servo, rendering.clone())
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
            last_size: Cell::new(rendering.size()),
            rendering,
        })
    }

    pub fn activate(&self, page: &ServoPage) {
        page.active.set(true);
        page.webview.show();
        page.webview.set_focused(true);
        page.needs_paint.set(true);
        self.egui_context.request_repaint();
    }

    pub fn deactivate(&self, page: &ServoPage) {
        page.active.set(false);
        page.webview.set_focused(false);
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

    pub fn render(
        &self,
        page: &ServoPage,
        size: PhysicalSize<u32>,
    ) -> Result<Option<egui::ColorImage>, String> {
        let size = PhysicalSize::new(size.width.max(1), size.height.max(1));
        if page.last_size.get() != size {
            page.webview.resize(size);
            page.last_size.set(size);
            page.needs_paint.set(true);
        }

        self.servo.spin_event_loop();
        if page.webview.animating() {
            // Keep animated pages on a steady frame cadence without busy-looping idle tabs.
            self.egui_context.request_repaint_after(std::time::Duration::from_millis(16));
        }
        if !page.needs_paint.replace(false) {
            return Ok(None);
        }

        page.rendering
            .make_current()
            .map_err(|error| format!("Servo GL context activation: {error:?}"))?;
        page.webview.paint();
        page.rendering.present();

        // Do not expose Servo's GL texture directly to egui. The renderers share
        // a context, but not a complete GL-state contract; a native texture can
        // leak stale/undefined contents into the desktop compositor. Copy the
        // completed frame into an egui-owned texture instead.
        let size = page.rendering.size();
        let rectangle = DeviceIntRect::new(
            DeviceIntPoint::new(0, 0),
            DeviceIntPoint::new(size.width as i32, size.height as i32),
        );
        let image = self
            .rendering
            .read_to_image(rectangle)
            .ok_or_else(|| "Could not read the rendered browser frame.".to_owned())?;
        let dimensions = [image.width() as usize, image.height() as usize];
        let pixels = image.into_raw();
        Ok(Some(egui::ColorImage::from_rgba_unmultiplied(
            dimensions,
            &pixels,
        )))
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

    pub fn forward_mouse_button(
        &self,
        page: &ServoPage,
        x: f32,
        y: f32,
        button: egui::PointerButton,
        pressed: bool,
    ) {
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
