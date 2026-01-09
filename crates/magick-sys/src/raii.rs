use crate::{
    AcquireExceptionInfo, DestroyExceptionInfo, DestroyImage, Errors, ExceptionInfo, Image,
    LockSemaphoreInfo, SemaphoreInfo, UnlockSemaphoreInfo,
};

#[repr(transparent)]
pub struct Exceptions(*mut ExceptionInfo);

impl Exceptions {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let exc = unsafe { AcquireExceptionInfo() };
        assert!(!exc.is_null());
        Self(exc)
    }

    pub unsafe fn catch<F: FnOnce(*mut ExceptionInfo) -> T, T>(
        &mut self,
        f: F,
    ) -> Result<T, Errors> {
        let ret = f(self.0);
        unsafe { crate::catch_exception(self.0) }.map(|()| ret)
    }
}

impl Drop for Exceptions {
    fn drop(&mut self) {
        unsafe {
            DestroyExceptionInfo(self.0);
        }
    }
}

#[repr(transparent)]
pub struct ImageHandle(*mut Image);

impl ImageHandle {
    pub unsafe fn from_raw(image: *mut Image) -> Self {
        assert!(!image.is_null());
        Self(image)
    }

    #[inline]
    pub unsafe fn as_ptr(&mut self) -> *mut Image { self.0 }

    #[inline]
    pub unsafe fn as_ref(&self) -> &Image { unsafe { &*self.0 } }
}

impl Drop for ImageHandle {
    fn drop(&mut self) {
        unsafe {
            DestroyImage(self.0);
        }
    }
}

#[repr(transparent)]
pub struct SemaphoreLock(*mut SemaphoreInfo);

impl SemaphoreLock {
    pub unsafe fn lock(sema: *mut SemaphoreInfo) -> Self {
        unsafe { LockSemaphoreInfo(sema) };
        Self(sema)
    }
}

impl Drop for SemaphoreLock {
    fn drop(&mut self) { unsafe { UnlockSemaphoreInfo(self.0) }; }
}
