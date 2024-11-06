use super::egl_ext::{self, InstanceExt};
use super::gl_ext::{self, GlExt};
use gbm::AsRaw;
use khronos_egl::{self as egl};
use pipewire::spa::param::video::VideoFormat;
use std::ffi::c_void;

#[derive(Debug)]
pub struct StringError {
    error: String,
}

impl StringError {
    fn new(error: &str) -> Self {
        Self {
            error: error.to_owned(),
        }
    }
}

impl std::fmt::Display for StringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl std::error::Error for StringError {}

#[derive(Debug)]
pub struct EglDmaBuf {
    egl: InstanceExt<egl::Static>,
    display: egl::Display,
    context: egl::Context,
    gbm_device: gbm::Device<std::fs::File>,
    gl_ext: GlExt,
}

impl Drop for EglDmaBuf {
    fn drop(&mut self) {
        gl_loader::end_gl();
    }
}

impl EglDmaBuf {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let drm_path = std::path::Path::new("/dev/dri/card1");
        let drm_fd = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(drm_path)?;
        let gbm_device = gbm::Device::new(drm_fd)?;

        println!("GBM backend: {}", gbm_device.backend_name());

        let egl = egl::Instance::new(egl::Static);
        let mut egl = InstanceExt::new(egl)?;

        let display = egl.get_playform_display_ext(
            egl_ext::EGL_PLATFORM_GBM_MESA,
            gbm_device.as_raw() as *mut c_void,
            None,
        )?;

        let (major, minor) = egl.initialize(display)?;
        println!("EGL initialized, version ({major}.{minor})");

        egl.bind_api(egl::OPENGL_API)?;

        let context = egl.create_context(
            display,
            unsafe { egl::Config::from_ptr(std::ptr::null_mut()) },
            None,
            &[egl::NONE],
        )?;

        egl.load_display_extensions(display)?;

        if gl_loader::init_gl() == 0 {
            return Err(StringError::new("Error load opengl library"))?;
        }

        gl::load_with(|symbol| gl_loader::get_proc_address(symbol) as *const _);

        let gl_ext = GlExt::load(&egl);

        Ok(Self {
            egl,
            display,
            context,
            gbm_device,
            gl_ext,
        })
    }

    pub fn image_from_dma_buf(
        &self,
        desktop_size: (u32, u32),
        format: pipewire::spa::param::video::VideoFormat,
        fds: &[i32],
        strides: &[u32],
        offsets: &[u32],
        modifier: u64,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if fds.is_empty() {
            return Err(StringError::new(
                "Failed to process buffer: invalid number of planes",
            ))?;
        }

        self.egl
            .make_current(self.display, None, None, Some(self.context))?;

        let mut image_attrs = Vec::with_capacity(47);
        image_attrs.push(egl::WIDTH);
        image_attrs.push(desktop_size.0 as _);
        image_attrs.push(egl::HEIGHT);
        image_attrs.push(desktop_size.1 as _);
        image_attrs.push(egl_ext::EGL_LINUX_DRM_FOURCC_EXT);
        image_attrs.push(spa_pixel_format_to_drm_format(format).unwrap());

        static FDS: [egl::Int; 4] = [
            egl_ext::EGL_DMA_BUF_PLANE0_FD_EXT,
            egl_ext::EGL_DMA_BUF_PLANE1_FD_EXT,
            egl_ext::EGL_DMA_BUF_PLANE2_FD_EXT,
            egl_ext::EGL_DMA_BUF_PLANE3_FD_EXT,
        ];
        static OFFSETS: [egl::Int; 4] = [
            egl_ext::EGL_DMA_BUF_PLANE0_OFFSET_EXT,
            egl_ext::EGL_DMA_BUF_PLANE1_OFFSET_EXT,
            egl_ext::EGL_DMA_BUF_PLANE2_OFFSET_EXT,
            egl_ext::EGL_DMA_BUF_PLANE3_OFFSET_EXT,
        ];
        static PITCHS: [egl::Int; 4] = [
            egl_ext::EGL_DMA_BUF_PLANE0_PITCH_EXT,
            egl_ext::EGL_DMA_BUF_PLANE1_PITCH_EXT,
            egl_ext::EGL_DMA_BUF_PLANE2_PITCH_EXT,
            egl_ext::EGL_DMA_BUF_PLANE3_PITCH_EXT,
        ];
        static MODIFIERS_LO: [egl::Int; 4] = [
            egl_ext::EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT,
            egl_ext::EGL_DMA_BUF_PLANE1_MODIFIER_LO_EXT,
            egl_ext::EGL_DMA_BUF_PLANE2_MODIFIER_LO_EXT,
            egl_ext::EGL_DMA_BUF_PLANE3_MODIFIER_LO_EXT,
        ];
        static MODIFIERS_HI: [egl::Int; 4] = [
            egl_ext::EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT,
            egl_ext::EGL_DMA_BUF_PLANE1_MODIFIER_HI_EXT,
            egl_ext::EGL_DMA_BUF_PLANE2_MODIFIER_HI_EXT,
            egl_ext::EGL_DMA_BUF_PLANE3_MODIFIER_HI_EXT,
        ];

        for idx in 0..fds.len() {
            image_attrs.push(FDS[idx]);
            image_attrs.push(fds[idx]);
            image_attrs.push(OFFSETS[idx]);
            image_attrs.push(offsets[idx] as _);
            image_attrs.push(PITCHS[idx]);
            image_attrs.push(strides[idx] as _);

            if gbm::Modifier::from(modifier) != gbm::Modifier::Invalid {
                image_attrs.push(MODIFIERS_LO[idx]);
                image_attrs.push((modifier & (0xFFFFFFFF as u64)).try_into().unwrap());
                image_attrs.push(MODIFIERS_HI[idx]);
                image_attrs.push((modifier >> 32) as _);
            }
        }

        image_attrs.push(egl::NONE);

        let image = self.egl.create_image_khr(
            &self.display,
            None,
            egl_ext::EGL_LINUX_DMA_BUF_EXT,
            None,
            Some(&image_attrs),
        )?;

        let mut texture = 0;
        unsafe {
            gl::GenTextures(1, &mut texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as _);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            self.gl_ext
                .gl_egl_image_target_texture_2does(gl::TEXTURE_2D, image.as_raw());
            gl_ext::check_error()?;
        }

        let gl_format = drm_pixel_format_to_gl(format);
        let mut src: Vec<u8> = Vec::with_capacity((strides[0] * desktop_size.1) as usize);

        unsafe {
            gl::GetTexImage(
                gl::TEXTURE_2D,
                0,
                gl_format,
                gl::UNSIGNED_BYTE,
                src.as_mut_ptr() as *mut c_void,
            );
            gl_ext::check_error()?;
            src.set_len(src.capacity());
            gl::DeleteTextures(1, &texture);
        };
        Ok(src)
    }

    // Note: This implementation does not work
    pub fn image_from_dma_buf_2(
        &self,
        desktop_size: (u32, u32),
        format: pipewire::spa::param::video::VideoFormat,
        fds: &[i32],
        strides: &[u32],
        offsets: &[u32],
        modifier: u64,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if fds.len() == 0 {
            return Err(StringError::new(
                "Failed to process buffer: invalid number of planes",
            ))?;
        }
        println!("1");
        println!("Modifier: {:?}", gbm::Modifier::from(modifier));
        let imported_object: gbm::BufferObject<()> = if modifier == gbm::Modifier::Invalid.into() {
            self.gbm_device.import_buffer_object_from_dma_buf(
                unsafe { std::os::fd::BorrowedFd::borrow_raw(fds[0]) },
                desktop_size.0,
                desktop_size.1,
                strides[0],
                gbm::Format::Argb8888,
                gbm::BufferObjectFlags::empty(),
            )
        } else {
            let buffers = iter_to_array(
                fds.iter()
                    .map(|&fd| Some(unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) })),
            );
            println!("1.5");
            self.gbm_device
                .import_buffer_object_from_dma_buf_with_modifiers(
                    fds.len() as u32,
                    buffers,
                    desktop_size.0,
                    desktop_size.1,
                    gbm::Format::Argb8888,
                    gbm::BufferObjectFlags::empty(),
                    iter_to_array(strides.iter().map(|&s| s as i32)),
                    iter_to_array(offsets.iter().map(|&o| o as i32)),
                    gbm::Modifier::from(modifier),
                )
        }?;
        println!("2");
        // bind context to render thread
        self.egl
            .make_current(self.display, None, None, Some(self.context))?;
        println!("3");

        // create EGL image from imported BO

        let client_buffer =
            unsafe { egl::ClientBuffer::from_ptr(imported_object.as_raw() as *mut _) };
        println!("3.5");
        let image = self.egl.create_image_khr(
            &self.display,
            None,
            egl_ext::EGL_NATIVE_PIXMAP_KHR,
            Some(&client_buffer),
            None,
        )?;
        println!("4");
        // create GL 2D texture for framebuffer

        let mut texture = 0;
        unsafe {
            gl::GenTextures(1, &mut texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as _);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            println!("4.5");
            self.gl_ext
                .gl_egl_image_target_texture_2does(gl::TEXTURE_2D, image.as_raw());
            gl_ext::check_error()?;
        }

        println!("5");

        let gl_format = drm_pixel_format_to_gl(format);

        let mut src: Vec<u8> = Vec::with_capacity((strides[0] * desktop_size.1) as usize);

        unsafe {
            gl::GetTexImage(
                gl::TEXTURE_2D,
                0,
                gl_format,
                gl::UNSIGNED_BYTE,
                src.as_mut_ptr() as *mut c_void,
            );

            gl_ext::check_error()?;

            src.set_len(src.capacity());
            println!("len: {}, src: {:?}", src.len(), &src[0..32]);

            gl::DeleteTextures(1, &texture);
        };
        Ok(src)
    }

    pub fn query_dma_buf_modifiers(
        &self,
        format: VideoFormat,
    ) -> Result<Vec<u64>, Box<dyn std::error::Error>> {
        let formats = self.egl.query_dma_buf_formats(&self.display)?;

        let drm_format = spa_pixel_format_to_drm_format(format).unwrap();

        formats
            .iter()
            .find(|&&f| f == drm_format)
            .ok_or(StringError::new("Format not supported for modifiers"))?;

        let mut modifiers = self
            .egl
            .query_dma_buf_modifiers_ext(&self.display, drm_format)?;

        modifiers.push(drm::buffer::DrmModifier::Invalid.into());
        Ok(modifiers)
    }
}

fn spa_pixel_format_to_drm_format(spa_format: VideoFormat) -> Option<i32> {
    use drm::buffer::DrmFourcc::*;
    match spa_format {
        VideoFormat::RGBA => Some(Abgr8888 as i32),
        VideoFormat::RGBx => Some(Xbgr8888 as i32),
        VideoFormat::BGRA => Some(Argb8888 as i32),
        VideoFormat::BGRx => Some(Xrgb8888 as i32),
        _ => None,
    }
}

fn drm_pixel_format_to_gl(format: pipewire::spa::param::video::VideoFormat) -> gl::types::GLenum {
    match format {
        VideoFormat::RGBx => gl::RGBA,
        VideoFormat::RGBA => gl::RGBA,
        VideoFormat::BGRx => gl::BGRA,
        VideoFormat::BGRA => gl::BGRA,
        VideoFormat::RGB => gl::RGB,
        VideoFormat::BGR => gl::BGR,
        _ => gl::BGRA,
    }
}

fn iter_to_array<const SIZE: usize, T: Default + Clone + Copy>(
    iter: impl Iterator<Item = T>,
) -> [T; SIZE] {
    let mut array = [T::default(); SIZE];
    iter.zip(array.iter_mut()).for_each(|(s, a)| *a = s);
    array
}

#[cfg(test)]
mod test {
    use super::EglDmaBuf;
    #[test]
    fn test() {
        let buf = EglDmaBuf::new();
        assert!(buf.is_ok());

        let buf = buf.unwrap();
        let modifiers = buf.query_dma_buf_modifiers(pipewire::spa::param::video::VideoFormat::RGBA);
        assert!(modifiers.is_ok());

        let modifiers = modifiers.unwrap();
        println!("modifiers {:#?}", modifiers);

        assert!(modifiers.len() > 0);
    }
}
