use std::{ffi::CStr, fmt};

use tracing::{error, warn};

use crate::{
    ClearMagickException, ExceptionInfo, ExceptionType_ErrorException,
    ExceptionType_FatalErrorException, ExceptionType_WarningException, GetNextValueInLinkedList,
    LinkedListInfo, MagickCoreSignature, ResetLinkedListIterator, SemaphoreLock,
};

#[derive(Debug, thiserror::Error)]
pub struct Error {
    pub fatal: bool,
    reason: String,
    description: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            fatal,
            reason,
            description,
        } = self;

        if *fatal {
            f.write_str("(FATAL) ")?;
        }

        if reason.is_empty() || description.is_empty() {
            write!(f, "{reason}{description}")
        } else {
            write!(f, "{reason} - {description}")
        }
    }
}

#[derive(Debug)]
pub enum Errors {
    One(Error),
    Many(Error, Box<Errors>),
}

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::One(e) => write!(f, "{e}"),
            Self::Many(e, es) => write!(f, "{e}\n{es}"),
        }
    }
}

impl std::error::Error for Errors {
    #[inline]
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::One(_) => None,
            Self::Many(_, e) => Some(e),
        }
    }
}

pub unsafe fn catch_exception(exception: *mut ExceptionInfo) -> Result<(), Errors> {
    assert!(!exception.is_null());
    assert!(unsafe { *exception }.signature == MagickCoreSignature as usize);

    let sema = unsafe { SemaphoreLock::lock((*exception).semaphore) };
    let mut errors = vec![];

    {
        let exception = &unsafe { *exception };

        if exception.exceptions.is_null() {
            return Ok(());
        }

        let exceptions = exception.exceptions.cast::<LinkedListInfo>();

        unsafe { ResetLinkedListIterator(exceptions) };
        loop {
            let p = unsafe { GetNextValueInLinkedList(exceptions) };
            if p.is_null() {
                break;
            }

            let exception = &unsafe { *p.cast::<ExceptionInfo>() };

            let info = || unsafe {
                (
                    if exception.reason.is_null() {
                        "".into()
                    } else {
                        CStr::from_ptr(exception.reason.cast_const()).to_string_lossy()
                    },
                    if exception.description.is_null() {
                        "".into()
                    } else {
                        CStr::from_ptr(exception.description.cast_const()).to_string_lossy()
                    },
                )
            };

            let error = match exception.severity {
                ..ExceptionType_WarningException => continue,
                ExceptionType_WarningException..ExceptionType_ErrorException => {
                    let (reason, description) = info();
                    warn!(%reason, %description);
                    continue;
                },
                ExceptionType_ErrorException..ExceptionType_FatalErrorException => {
                    let (reason, description) = info();
                    error!(%reason, %description, fatal = false);
                    Error {
                        fatal: false,
                        reason: reason.into_owned(),
                        description: description.into_owned(),
                    }
                },
                ExceptionType_FatalErrorException.. => {
                    let (reason, description) = info();
                    error!(%reason, %description, fatal = true);
                    Error {
                        fatal: true,
                        reason: reason.into_owned(),
                        description: description.into_owned(),
                    }
                },
            };

            errors.push(error);
        }
    }

    drop(sema);
    unsafe { ClearMagickException(exception) };

    let mut res = Ok(());

    while let Some(err) = errors.pop() {
        res = match res {
            Ok(()) => Err(Errors::One(err)),
            Err(e) => Err(Errors::Many(err, e.into())),
        };
    }

    res
}
