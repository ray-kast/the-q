use std::io::prelude::*;

use crate::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error")]
    Io(#[from] std::io::Error),
    #[error("Error decoding message payload")]
    Protobuf(#[from] prost::DecodeError),
}

impl From<Infallible> for Error {
    fn from(value: Infallible) -> Self { match value {} }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id<'a>(Cow<'a, str>);

impl<'a> fmt::Display for Id<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
}

impl<'a> Id<'a> {
    pub fn as_ref(&self) -> Id<'_> { Id(Cow::Borrowed(self.0.as_ref())) }
}

const FORMAT_RAW: u8 = 0;
const FORMAT_ZSTD: u8 = 1;

const ZSTD_WINDOW_LOG: u32 = 10; // The minimum, but larger than our target payload size

// TODO: was io::{Read, Write} the correct abstraction for b64k?
pub fn read<M: prost::Message + Default>(i: &Id<'_>) -> Result<M, Error> {
    let mut dec = base64k::Decoder::new(i.0.chars());

    let mut fmt_buf = [0];
    match dec.read_exact(&mut fmt_buf) {
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(M::default()),
        r => r.unwrap(),
    }

    // Format byte
    let mut msg_buf = vec![];
    match fmt_buf[0] {
        FORMAT_RAW => {
            dec.read_to_end(&mut msg_buf).unwrap();
        },
        FORMAT_ZSTD => {
            let mut z_dec = zstd::stream::Decoder::new(dec)?;
            z_dec.include_magicbytes(false)?;
            z_dec.window_log_max(ZSTD_WINDOW_LOG)?;
            z_dec.read_to_end(&mut msg_buf)?;
        },
        _ => return Ok(M::default()),
    }

    M::decode(&*msg_buf).map_err(Error::Protobuf)
}

pub fn write(id: &impl prost::Message) -> Result<Id<'static>, Error> {
    let raw = id.encode_to_vec();

    let mut z_enc = zstd::stream::Encoder::new(vec![], 22)?; // TODO
    z_enc.include_magicbytes(false)?;
    z_enc.include_checksum(false)?;
    z_enc.set_pledged_src_size(Some(raw.len().try_into().unwrap()))?;
    z_enc.window_log(ZSTD_WINDOW_LOG)?;
    z_enc.write_all(&raw)?;
    let compressed = z_enc.finish()?;

    let mut enc = base64k::Encoder::default();
    let format = if compressed.len() < raw.len() {
        trace!(?id, "Selecting zstd format for ID");
        FORMAT_ZSTD
    } else {
        trace!(?id, "Selecting raw format for ID");
        FORMAT_RAW
    };

    enc.write_all(&[format]).unwrap();

    match format {
        FORMAT_RAW => enc.write_all(&raw).unwrap(),
        FORMAT_ZSTD => enc.write_all(&compressed).unwrap(),
        _ => unreachable!(),
    }

    Ok(Id(Cow::Owned(enc.finish())))
}

#[cfg(test)]
mod test {
    use crate::prelude::*;

    #[derive(prost::Message)]
    struct Msg {
        #[prost(string, tag = "1")]
        s: String,
    }

    #[test]
    fn test_roundtrip() -> Result {
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
