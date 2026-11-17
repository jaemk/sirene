//! Reading diagram sources from files and streams (WORKSP-10).

use std::io::Read;
use std::path::Path;

use crate::{Error, Result};

/// The largest source read, in bytes (64 MiB).
pub const MAX_INPUT: u64 = 64 << 20;

/// Reads at most [`MAX_INPUT`] bytes from `r`, or an error past that.
pub fn read_bytes(r: impl Read) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    r.take(MAX_INPUT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::with_source("cannot read input bytes", e))?;
    if bytes.len() as u64 > MAX_INPUT {
        return Err(Error::new(format!(
            "the text is over {} MiB",
            MAX_INPUT >> 20
        )));
    }
    Ok(bytes)
}

/// `bytes` as UTF-8, or an error naming UTF-16 (by its byte order mark) or the first bad byte.
pub fn text(bytes: Vec<u8>) -> Result<String> {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        return Err("the text is UTF-16; sirene reads UTF-8".into());
    }
    String::from_utf8(bytes).map_err(|e| {
        let at = e.utf8_error().valid_up_to();
        Error::with_source(
            format!("the text is not UTF-8 (an invalid byte at offset {at})"),
            e,
        )
    })
}

/// Reads `r` as UTF-8 text of at most [`MAX_INPUT`] bytes.
pub fn read_text(r: impl Read) -> Result<String> {
    read_bytes(r).and_then(text)
}

/// Reads the file at `path` as UTF-8 text of at most [`MAX_INPUT`] bytes, with errors that name
/// it. Pass `path` to [`crate::diagram::Lang::of_path`] for the language its extension names.
pub fn read(path: &Path) -> Result<String> {
    let context = || format!("cannot read {}", path.display());
    let file = std::fs::File::open(path).map_err(|e| Error::with_source(context(), e))?;
    read_text(file).map_err(|e| e.context(context()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ERRORH-1: original I/O and UTF-8 errors survive through file context.
    #[test]
    fn read_errors_retain_their_causes() {
        use std::error::Error as _;

        let dir = std::env::temp_dir().join(format!("sirene-source-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("missing.mmd");
        let error = read(&missing).unwrap_err();
        let io = error
            .source()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .unwrap();
        assert_eq!(io.kind(), std::io::ErrorKind::NotFound);
        assert!(error.report().contains(&missing.display().to_string()));

        let path = dir.join("bad.mmd");
        std::fs::write(&path, b"a\xff").unwrap();
        let error = read(&path).unwrap_err();
        assert!(error.report().contains("offset 1"));
        let encoding = error.source().unwrap().downcast_ref::<Error>().unwrap();
        assert!(
            encoding
                .source()
                .unwrap()
                .is::<std::string::FromUtf8Error>()
        );

        let path = dir.join("good.mmd");
        std::fs::write(&path, "flowchart\nA\n").unwrap();
        assert_eq!(read(&path).unwrap(), "flowchart\nA\n");
        std::fs::remove_dir_all(&dir).unwrap();

        struct FailedRead;
        impl Read for FailedRead {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::PermissionDenied.into())
            }
        }
        let error = read_bytes(FailedRead).unwrap_err();
        assert_eq!(
            error
                .source()
                .unwrap()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn reads_are_capped() {
        let big = std::io::repeat(b'a').take(MAX_INPUT + 1);
        assert!(
            read_bytes(big)
                .unwrap_err()
                .report()
                .contains("over 64 MiB")
        );
        assert_eq!(read_bytes(&b"ab"[..]).unwrap(), b"ab");
    }

    #[test]
    fn utf16_is_named() {
        for bom in [[0xff, 0xfe], [0xfe, 0xff]] {
            let error = read_text(&bom[..]).unwrap_err();
            assert!(error.report().contains("UTF-16"), "{}", error.report());
        }
        assert_eq!(read_text(&b"pie\n"[..]).unwrap(), "pie\n");
    }
}
