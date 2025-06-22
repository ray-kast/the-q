//! Support code for encoding and decoding compact custom IDs

use std::{borrow::Cow, convert::Infallible, fmt, io::prelude::*};

/// An error occurring from transcoding a custom ID
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An I/O error
    #[error("IO error")]
    Io(#[from] std::io::Error),
    /// A protobuf error originating from [`prost`]
    #[error("Error decoding message payload")]
    Protobuf(#[from] prost::DecodeError),
}

impl From<Infallible> for Error {
    fn from(value: Infallible) -> Self { match value {} }
}

/// An encoded custom ID, using a protobuf payload encoded with [`base64k`] and
/// compressed
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id<'a>(Cow<'a, str>);

impl fmt::Display for Id<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
}

impl<'a> Id<'a> {
    /// Construct a new custom ID from its raw string representation
    ///
    /// # Safety
    /// It is up to the caller to ensure that the provided string is a valid ID
    /// payload encoded by the [`write()`] function.
    #[must_use]
    pub unsafe fn from_inner(s: Cow<'a, str>) -> Self { Self(s) }

    /// Produce an ID that borrows from `self`
    #[must_use]
    pub fn as_ref(&self) -> Id<'_> { Id(Cow::Borrowed(self.0.as_ref())) }
}

const FORMAT_RAW: u8 = 0;
const FORMAT_ZSTD: u8 = 1;

const ZSTD_WINDOW_LOG: u32 = 10; // The minimum, but larger than our target payload size

/// Decode the given [`Id`] into a custom ID message
///
/// # Errors
/// This function returns an error if an unrecoverable format error occurs while
/// reading the input data.
// TODO: was io::{Read, Write} the correct abstraction for b64k?
pub fn read<M: prost::Message + Default>(i: &Id<'_>) -> Result<M, Error> {
    let mut dec = base64k::Decoder::new(i.0.chars());

    let mut fmt_buf = [0];
    match dec.read_exact(&mut fmt_buf) {
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(M::default()),
        r => r?,
    }

    // Format byte
    let mut msg_buf = vec![];
    match fmt_buf[0] {
        FORMAT_RAW => {
            dec.read_to_end(&mut msg_buf)?;
        },
        FORMAT_ZSTD => {
            let mut z_dec = zstd::stream::Decoder::new(dec)?;
            z_dec.include_magicbytes(false)?;
            z_dec.window_log_max(ZSTD_WINDOW_LOG)?;
            z_dec.read_to_end(&mut msg_buf)?;
        },
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Unrecognized format byte",
            )
            .into());
        },
    }

    M::decode(&*msg_buf).map_err(Error::Protobuf)
}

/// Encode the given message into an [`Id`]
///
/// # Errors
/// This function fails if an unrecoverable format error occurs while writing
/// the output string.
pub fn write<I: fmt::Debug + prost::Message>(id: &I) -> Result<Id<'static>, Error> {
    let raw = id.encode_to_vec();

    let mut z_enc = zstd::stream::Encoder::new(vec![], 22)?; // TODO
    z_enc.include_magicbytes(false)?;
    z_enc.include_checksum(false)?;
    z_enc.set_pledged_src_size(raw.len().try_into().ok())?;
    z_enc.window_log(ZSTD_WINDOW_LOG)?;
    z_enc.write_all(&raw)?;
    let compressed = z_enc.finish()?;

    let mut enc = base64k::Encoder::default();
    let format = if compressed.len() < raw.len() {
        tracing::trace!(?id, "Selecting zstd format for ID");
        FORMAT_ZSTD
    } else {
        tracing::trace!(?id, "Selecting raw format for ID");
        FORMAT_RAW
    };

    enc.write_all(&[format])?;

    match format {
        FORMAT_RAW => enc.write_all(&raw)?,
        FORMAT_ZSTD => enc.write_all(&compressed)?,
        _ => unreachable!(),
    }

    Ok(Id(Cow::Owned(enc.finish())))
}

#[cfg(test)]
mod test {
    use anyhow::Context as _;

    #[derive(prost::Message)]
    struct Msg {
        #[prost(string, tag = "1")]
        s: String,
    }

    #[test]
    fn test_roundtrip() -> Result<(), anyhow::Error> {
        let s = "1234";
        let id = super::write(&Msg { s: s.to_owned() }).context("Error writing short message")?;

        let Msg { s: s2 } = super::read(&id).context("Error reading short message")?;
        assert_eq!(s, s2);

        let mut s = String::new();
        (0..1000).for_each(|_| s.push_str("0123456789"));
        let id = super::write(&Msg { s: s.clone() }).context("Error writing long message")?;

        let Msg { s: s2 } = super::read(&id).context("Error reading long message")?;
        assert_eq!(s, s2);
        Ok(())
    }
}
