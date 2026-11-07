use std::cell::Cell;
use std::ffi::{CStr, CString, c_void};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;

use eframe::glow;
use gleam::gl::{self, Gl};
use servo::{RgbaImage, RenderingContext};
use winit::dpi::PhysicalSize;
use webrender_api::units::DeviceIntRect;

pub(super) struct EguiGlRenderingContext {
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

    pub(super) fn new(
        glow_gl: Arc<glow::Context>,
        get_proc_address: Arc<dyn Fn(&CStr) -> *const c_void + Send + Sync>,
        size: PhysicalSize<u32>,
    ) -> Result<Self, String> {
        let size = PhysicalSize::new(size.width.max(1), size.height.max(1));
        let gleam_gl = Self::load_gleam_api(get_proc_address);

        // Page surfaces are created while egui is drawing. Restore every
        // binding we touch so creating a tab cannot redirect the desktop frame.
        let mut previous_framebuffer = [0];
        let mut previous_texture = [0];
        let mut previous_renderbuffer = [0];
        #[expect(unsafe_code)]
        unsafe {
            gleam_gl.get_integer_v(gl::FRAMEBUFFER_BINDING, &mut previous_framebuffer);
            gleam_gl.get_integer_v(gl::TEXTURE_BINDING_2D, &mut previous_texture);
            gleam_gl.get_integer_v(gl::RENDERBUFFER_BINDING, &mut previous_renderbuffer);
        }

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
        gleam_gl.bind_framebuffer(gl::FRAMEBUFFER, previous_framebuffer[0].max(0) as u32);
        gleam_gl.bind_texture(gl::TEXTURE_2D, previous_texture[0].max(0) as u32);
        gleam_gl.bind_renderbuffer(gl::RENDERBUFFER, previous_renderbuffer[0].max(0) as u32);
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
        // Deleting a framebuffer that is still bound implicitly switches the
        // context to framebuffer 0. Restore the target that was active before
        // Servo began rendering instead; never blindly redirect eframe to 0.
        let mut bound_framebuffer = [0];
        #[expect(unsafe_code)]
        unsafe {
            self.gleam_gl
                .get_integer_v(gl::FRAMEBUFFER_BINDING, &mut bound_framebuffer);
        }
        if bound_framebuffer[0].max(0) as u32 == self.framebuffer_id {
            self.gleam_gl.bind_framebuffer(
                gl::FRAMEBUFFER,
                self.previous_framebuffer.get(),
            );
        }

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

        let mut previous_texture = [0];
        let mut previous_renderbuffer = [0];
        #[expect(unsafe_code)]
        unsafe {
            self.gleam_gl.get_integer_v(gl::TEXTURE_BINDING_2D, &mut previous_texture);
            self.gleam_gl.get_integer_v(gl::RENDERBUFFER_BINDING, &mut previous_renderbuffer);
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
        self.gleam_gl.bind_texture(gl::TEXTURE_2D, previous_texture[0].max(0) as u32);
        self.gleam_gl.bind_renderbuffer(gl::RENDERBUFFER, self.renderbuffer_id);
        self.gleam_gl.renderbuffer_storage(
            gl::RENDERBUFFER,
            gl::DEPTH_COMPONENT24,
            size.width as gl::GLsizei,
            size.height as gl::GLsizei,
        );
        self.gleam_gl.bind_renderbuffer(gl::RENDERBUFFER, previous_renderbuffer[0].max(0) as u32);
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
