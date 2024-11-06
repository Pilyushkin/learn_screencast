use khronos_egl as egl;
use std::error::Error;
use std::fmt::Display;

#[derive(Debug)]
pub struct GlExt {
    gl_egl_image_target_texture_2does: sys::GlEGLImageTargetTexture2DOES,
}

impl GlExt {
    pub fn load<T: egl::api::EGL1_0>(loader: &egl::Instance<T>) -> Self {
        let gl_egl_image_target_texture_2does = unsafe {
            std::mem::transmute::<extern "system" fn(), sys::GlEGLImageTargetTexture2DOES>(
                loader
                    .get_proc_address("glEGLImageTargetTexture2DOES")
                    .unwrap(),
            )
        };

        Self {
            gl_egl_image_target_texture_2does,
        }
    }

    pub fn gl_egl_image_target_texture_2does(
        &self,
        target: gl::types::GLenum,
        image: sys::GLeglImageOES,
    ) {
        (self.gl_egl_image_target_texture_2does)(target, image);
    }
}

#[derive(Debug)]
pub struct GlError(gl::types::GLenum);

impl Display for GlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            gl::NO_ERROR => write!(f, "Ok"),
            gl::INVALID_ENUM => write!(f, "Invalid enum"),
            gl::INVALID_VALUE => write!(f, "Invalid value"),
            gl::INVALID_OPERATION => write!(f, "Invalid operation"),
            gl::STACK_OVERFLOW => write!(f, "Stack overflow"),
            gl::STACK_UNDERFLOW => write!(f, "Stack underflow"),
            gl::OUT_OF_MEMORY => write!(f, "Out of memory"),
            gl::INVALID_FRAMEBUFFER_OPERATION => write!(f, "Invalid framebuffer operation"),
            e @ _ => write!(f, "Unrecognized: {}", e),
        }
    }
}

impl Error for GlError {}

pub fn check_error() -> Result<(), GlError> {
    let error = unsafe { gl::GetError() };
    if error == gl::NO_ERROR {
        return Ok(());
    }

    Err(GlError(error))
}

mod sys {
    use std::ffi::c_void;

    pub type GLeglImageOES = *const c_void;
    pub type GlEGLImageTargetTexture2DOES = fn(target: gl::types::GLenum, image: GLeglImageOES);
}
