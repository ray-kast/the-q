use crate::{
    _LqrRetVal, _LqrRetVal_LQR_ERROR, _LqrRetVal_LQR_NOMEM, _LqrRetVal_LQR_OK,
    _LqrRetVal_LQR_USRCANCEL,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Out of memory")]
    OutOfMemory,
    #[error("Operation canceled by user")]
    Canceled,
    #[error("Unspecified liblqr error")]
    Other,
    #[error("Invalid integer conversion")]
    TryFromInt(#[from] std::num::TryFromIntError),
}

impl Error {
    pub fn from_code(code: _LqrRetVal) -> Result<(), Self> {
        match code {
            _LqrRetVal_LQR_ERROR => Err(Self::Other),
            _LqrRetVal_LQR_OK => Ok(()),
            _LqrRetVal_LQR_NOMEM => Err(Self::OutOfMemory),
            _LqrRetVal_LQR_USRCANCEL => Err(Self::Canceled),
            _ => unreachable!(),
        }
    }
}
