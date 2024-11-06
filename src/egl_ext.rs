use khronos_egl::{self as egl};
use std::{
    ffi::c_void,
    ops::{Deref, DerefMut},
};

pub const EGL_PLATFORM_GBM_MESA: egl::Enum = 0x31D7;
pub const EGL_NATIVE_PIXMAP_KHR: egl::Enum = 0x30B0;
pub const EGL_PLATFORM_GBM_KHR: egl::Enum = 0x31D7;

pub const EGL_LINUX_DRM_FOURCC_EXT: egl::Int = 0x3271;

pub const EGL_DMA_BUF_PLANE0_FD_EXT: egl::Int = 0x3272;
pub const EGL_DMA_BUF_PLANE0_OFFSET_EXT: egl::Int = 0x3273;
pub const EGL_DMA_BUF_PLANE0_PITCH_EXT: egl::Int = 0x3274;

pub const EGL_DMA_BUF_PLANE1_FD_EXT: egl::Int = 0x3275;
pub const EGL_DMA_BUF_PLANE1_OFFSET_EXT: egl::Int = 0x3276;
pub const EGL_DMA_BUF_PLANE1_PITCH_EXT: egl::Int = 0x3277;

pub const EGL_DMA_BUF_PLANE2_FD_EXT: egl::Int = 0x3278;
pub const EGL_DMA_BUF_PLANE2_OFFSET_EXT: egl::Int = 0x3279;
pub const EGL_DMA_BUF_PLANE2_PITCH_EXT: egl::Int = 0x327A;

pub const EGL_DMA_BUF_PLANE3_FD_EXT: egl::Int = 0x3440;
pub const EGL_DMA_BUF_PLANE3_OFFSET_EXT: egl::Int = 0x3441;
pub const EGL_DMA_BUF_PLANE3_PITCH_EXT: egl::Int = 0x3442;

pub const EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT: egl::Int = 0x3443;
pub const EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT: egl::Int = 0x3444;

pub const EGL_DMA_BUF_PLANE3_MODIFIER_LO_EXT: egl::Int = 0x3449;
pub const EGL_DMA_BUF_PLANE3_MODIFIER_HI_EXT: egl::Int = 0x344A;

pub const EGL_DMA_BUF_PLANE1_MODIFIER_LO_EXT: egl::Int = 0x3445;
pub const EGL_DMA_BUF_PLANE1_MODIFIER_HI_EXT: egl::Int = 0x3446;
pub const EGL_DMA_BUF_PLANE2_MODIFIER_LO_EXT: egl::Int = 0x3447;
pub const EGL_DMA_BUF_PLANE2_MODIFIER_HI_EXT: egl::Int = 0x3448;

pub const EGL_LINUX_DMA_BUF_EXT: egl::Enum = 0x3270;

#[derive(Debug)]
pub struct InstanceExt<T> {
    instance: egl::Instance<T>,
    egl_get_playform_display_ext: sys::EglGetPlatformDisplayEXT,
    egl_create_image_khr: sys::EglCreateImageKHR,
    egl_destroy_image: sys::EglDestroyImageKHR,
    egl_query_dma_buf_formats: Option<sys::EglQueryDmaBufFormatsEXT>,
    egl_query_dma_buf_modifiers_formats: Option<sys::EglQueryDmaBufModifiersEXT>,
}

impl<T> Deref for InstanceExt<T> {
    type Target = egl::Instance<T>;

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

impl<T> DerefMut for InstanceExt<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.instance
    }
}

impl<T: egl::api::EGL1_5> InstanceExt<T> {
    pub fn new(instance: egl::Instance<T>) -> Result<Self, Box<dyn std::error::Error>> {
        // check no display extensions

        let mut client_extensions_no_display = instance
            .query_string(None, egl::EXTENSIONS)?
            .to_str()?
            .split(' ');

        let has_platform_base_ext = client_extensions_no_display
            .find(|&i| i == "EGL_EXT_platform_base")
            .is_some();

        let has_platform_gbm_ext = client_extensions_no_display
            .find(|&i| i == "EGL_MESA_platform_gbm")
            .is_some();

        // let has_khr_platform_gbm = client_extensions_no_display // for EGL_PLATFORM_GBM_KHR
        //     .find(|&i| i == "EGL_KHR_platform_gbm")
        //     .is_some();

        if !has_platform_base_ext || !has_platform_gbm_ext
        /*|| !has_khr_platform_gbm*/
        {
            return Err(EglExtError::NoNoDisplayExtensions)?;
        }

        let egl_get_playform_display_ext = instance
            .get_proc_address("eglGetPlatformDisplayEXT")
            .unwrap();
        let egl_get_playform_display_ext = unsafe {
            std::mem::transmute::<extern "system" fn(), sys::EglGetPlatformDisplayEXT>(
                egl_get_playform_display_ext,
            )
        };

        let egl_create_image_khr = unsafe {
            std::mem::transmute::<extern "system" fn(), sys::EglCreateImageKHR>(
                instance.get_proc_address("eglCreateImageKHR").unwrap(),
            )
        };

        let egl_destroy_image = unsafe {
            std::mem::transmute::<extern "system" fn(), sys::EglDestroyImageKHR>(
                instance.get_proc_address("eglDestroyImageKHR").unwrap(),
            )
        };

        Ok(Self {
            instance,
            egl_get_playform_display_ext,
            egl_create_image_khr,
            egl_destroy_image,
            egl_query_dma_buf_formats: None,
            egl_query_dma_buf_modifiers_formats: None,
        })
    }

    pub fn get_playform_display_ext(
        &self,
        platform: egl::Enum,
        native_display: *mut c_void,
        attrib_list: Option<&[egl::Attrib]>,
    ) -> Result<egl::Display, Box<dyn std::error::Error>> {
        let raw_display = (self.egl_get_playform_display_ext)(
            platform,
            native_display,
            attrib_list.map(|a| a.as_ptr()).unwrap_or(std::ptr::null()),
        );

        if raw_display == egl::NO_DISPLAY {
            return Err(self.instance.get_error().unwrap())?; // EglGetError
        }

        Ok(unsafe { egl::Display::from_ptr(raw_display) })
    }

    pub fn load_display_extensions(
        &mut self,
        display: egl::Display,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client_extensions_display = self
            .instance
            .query_string(Some(display), egl::EXTENSIONS)?
            .to_str()?
            .split(' ');

        let has_image_dma_buf_import_ext = client_extensions_display
            .find(|&i| i == "EGL_EXT_image_dma_buf_import")
            .is_some();

        if has_image_dma_buf_import_ext {
            let func = unsafe {
                std::mem::transmute::<extern "system" fn(), sys::EglQueryDmaBufFormatsEXT>(
                    self.instance
                        .get_proc_address("eglQueryDmaBufFormatsEXT")
                        .expect("Unrichable eglQueryDmaBufFormatsEXT"),
                )
            };
            self.egl_query_dma_buf_formats = Some(func);
        }

        let has_image_dma_buf_import_modifiers_ext = client_extensions_display
            .find(|&i| i == "EGL_EXT_image_dma_buf_import_modifiers")
            .is_some();

        if has_image_dma_buf_import_modifiers_ext {
            let func = unsafe {
                std::mem::transmute::<extern "system" fn(), sys::EglQueryDmaBufModifiersEXT>(
                    self.instance
                        .get_proc_address("eglQueryDmaBufModifiersEXT")
                        .expect("Unrichable eglQueryDmaBufModifiersEXT"),
                )
            };
            self.egl_query_dma_buf_modifiers_formats = Some(func);
        }
        Ok(())
    }

    pub fn query_dma_buf_formats(
        &self,
        dpy: &egl::Display,
    ) -> Result<Vec<egl::Int>, Box<dyn std::error::Error>> {
        if let Some(func) = self.egl_query_dma_buf_formats {
            let mut count = 0;
            let success = func(dpy.as_ptr(), 0, std::ptr::null_mut(), &mut count);
            if success == 0 || count <= 0 {
                return Err(EglExtError::NoDmaBufFormats)?;
            }
            let mut formats = vec![0; count as usize];
            if func(dpy.as_ptr(), count, formats.as_mut_ptr(), &mut count) != 0 {
                return Ok(formats);
            }
            return Err(EglExtError::NoDmaBufFormats)?;
        }
        return Err(EglExtError::ExtensionUnavailable)?;
    }

    pub fn query_dma_buf_modifiers_ext(
        &self,
        dpy: &egl::Display,
        format: egl::Int,
    ) -> Result<Vec<u64>, Box<dyn std::error::Error>> {
        if let Some(func) = self.egl_query_dma_buf_modifiers_formats {
            let mut count = 0;
            let success = func(
                dpy.as_ptr(),
                format,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut count,
            );
            if success == 0 || count <= 0 {
                return Err(EglExtError::NoDmaBufModifiers)?;
            }

            let mut modifiers = vec![0; count as usize];
            if 0 == func(
                dpy.as_ptr(),
                format as i32,
                count,
                modifiers.as_mut_ptr(),
                std::ptr::null_mut(),
                &mut count,
            ) {
                return Err(EglExtError::NoDmaBufModifiers)?;
            }
            return Ok(modifiers);
        }
        return Err(EglExtError::NoDmaBufModifiers)?;
    }

    pub fn create_image_khr<'a, 'b>(
        &'b self,
        dpy: &'a egl::Display,
        ctx: Option<&egl::Context>,
        target: egl::Enum,
        buffer: Option<&egl::ClientBuffer>,
        attrib_list: Option<&[egl::Int]>,
    ) -> Result<EGLImageKHR<'a, 'b, T>, Box<dyn std::error::Error>> {
        let image = (self.egl_create_image_khr)(
            dpy.as_ptr(),
            ctx.map(|c| c.as_ptr()).unwrap_or(egl::NO_CONTEXT),
            target,
            buffer.map(|b| b.as_ptr()).unwrap_or(std::ptr::null_mut()),
            attrib_list.map(|a| a.as_ptr()).unwrap_or(std::ptr::null()),
        );
        if image == std::ptr::null_mut() {
            return Err(self.instance.get_error().unwrap())?;
        }
        Ok(EGLImageKHR {
            image,
            dpy,
            instance: self,
        })
    }
}

pub struct EGLImageKHR<'a, 'b, T> {
    image: sys::EGLImageKHR,
    dpy: &'a egl::Display,
    instance: &'b InstanceExt<T>,
}

impl<'a, 'b, T> Drop for EGLImageKHR<'a, 'b, T> {
    fn drop(&mut self) {
        (self.instance.egl_destroy_image)(self.dpy.as_ptr(), self.image);
    }
}

impl<'a, 'b, T> EGLImageKHR<'a, 'b, T> {
    pub fn as_raw(&self) -> sys::EGLImageKHR {
        self.image
    }
}

#[derive(Debug)]
enum EglExtError {
    NoNoDisplayExtensions,
    NoDisplayExtensions,
    EglGetError,
    ExtensionUnavailable,
    NoDmaBufFormats,
    NoDmaBufModifiers,
}

impl std::fmt::Display for EglExtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EglExtError::NoNoDisplayExtensions => {
                write!(f, "One of required EGL no display extensions is missing")
            }

            EglExtError::NoDisplayExtensions => {
                write!(f, "One of required EGL display extensions is missing")
            }

            EglExtError::EglGetError => write!(f, "Failed get error grom egl::get_error()"),

            EglExtError::ExtensionUnavailable => write!(f, "Extension unavailable"),

            EglExtError::NoDmaBufFormats => write!(f, "Failed get dma buf formats"),

            EglExtError::NoDmaBufModifiers => write!(f, "Failed get dma buf modifiers"),
        }
    }
}

impl std::error::Error for EglExtError {}

mod sys {
    use khronos_egl as egl;
    use std::ffi::{c_int, c_void};

    pub type EglGetPlatformDisplayEXT = fn(
        platform: egl::Enum,
        native_display: *mut c_void,
        attrib_list: *const egl::Attrib,
    ) -> egl::EGLDisplay;

    pub type EglQueryDmaBufFormatsEXT = fn(
        dpy: egl::EGLDisplay,
        max_formats: egl::Int,
        formats: *mut egl::Int,
        num_formats: *mut egl::Int,
    ) -> egl::Boolean;

    pub type EglQueryDmaBufModifiersEXT = fn(
        dpy: egl::EGLDisplay,
        format: egl::Int,
        max_modifires: egl::Int,
        modifires: *mut u64,
        external_only: *mut bool,
        num_modifires: *mut c_int,
    ) -> egl::Boolean;

    pub type EGLImageKHR = *mut c_void;
    pub type EglCreateImageKHR = fn(
        dpy: egl::EGLDisplay,
        ctx: egl::EGLContext,
        target: egl::Enum,
        buffer: egl::EGLClientBuffer,
        attrib_list: *const egl::Int,
    ) -> EGLImageKHR;

    pub type EglDestroyImageKHR = fn(dpy: egl::EGLDisplay, image: EGLImageKHR);
}
