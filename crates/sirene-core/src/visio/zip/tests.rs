use super::*;
use crate::visio::testpkg::{Entry, stored, zip};

fn deflated(name: &str, data: &[u8]) -> Entry {
    Entry {
        name: name.to_owned(),
        data: data.to_vec(),
        deflate: true,
    }
}

#[test]
fn crc32_check_value() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b""), 0);
}

#[test]
fn reads_stored_and_deflated_entries() {
    let long = "visio ".repeat(500);
    let bytes = zip(&[
        stored("a.txt", b"hello"),
        deflated("dir/B.txt", long.as_bytes()),
        stored("dir/", b""),
    ]);
    let mut archive = Archive::open(&bytes).unwrap();
    assert_eq!(archive.names(), ["a.txt", "dir/b.txt"]);
    assert_eq!(archive.read("a.txt").unwrap(), b"hello");
    assert_eq!(archive.read("DIR/b.TXT").unwrap(), long.as_bytes());
    assert!(archive.contains("A.TXT"));
    assert!(archive.read("missing").is_err());
}

#[test]
fn rejects_non_archives() {
    for bytes in [
        &b""[..],
        b"PK",
        b"not a zip at all, just text",
        b"PK\x03\x04junk",
    ] {
        assert!(Archive::open(bytes).is_err(), "{bytes:?}");
    }
}

#[test]
fn rejects_truncation_at_every_length() {
    let bytes = zip(&[stored("a.txt", b"hello"), deflated("b.txt", &[7u8; 2000])]);
    for len in 0..bytes.len() {
        let cut = bytes.get(..len).unwrap();
        // Never a panic; a cut archive is an error or, if the cut spared what was asked, reads.
        if let Ok(mut a) = Archive::open(cut) {
            let _ = a.read("a.txt");
            let _ = a.read("b.txt");
        }
    }
    assert!(Archive::open(bytes.get(..bytes.len() - 1).unwrap()).is_err());
}

#[test]
fn rejects_a_bad_checksum() {
    let mut bytes = zip(&[stored("a.txt", b"hello")]);
    // The data follows the 30 byte header and the name.
    let at = 30 + "a.txt".len();
    *bytes.get_mut(at).unwrap() = b'j';
    let mut archive = Archive::open(&bytes).unwrap();
    assert!(
        archive
            .read("a.txt")
            .unwrap_err()
            .report()
            .contains("checksum")
    );
}

#[test]
fn rejects_unsafe_names() {
    for name in [
        "../evil.xml",
        "a/../../evil.xml",
        "/abs.xml",
        "a\\b.xml",
        "C:/x.xml",
        "a//b.xml",
        "a/./..",
    ] {
        let bytes = zip(&[stored(name, b"x")]);
        let e = Archive::open(&bytes).unwrap_err();
        assert!(
            e.report().contains("unsafe entry name"),
            "{name}: {}",
            e.report()
        );
    }
    assert!(safe_name("visio/pages/page1.xml"));
    assert!(safe_name("[Content_Types].xml"));
    assert!(!safe_name(""));
}

#[test]
fn rejects_duplicate_names() {
    let bytes = zip(&[stored("a.xml", b"1"), stored("A.XML", b"2")]);
    assert!(
        Archive::open(&bytes)
            .unwrap_err()
            .report()
            .contains("duplicate")
    );
}

#[test]
fn rejects_a_zip_bomb() {
    // 8 MiB of zeros deflates to a few KiB: far over the 200 to 1 ratio.
    let bytes = zip(&[deflated("bomb.xml", &vec![0u8; 8 * 1024 * 1024])]);
    let e = Archive::open(&bytes).unwrap_err();
    assert!(e.report().contains("expands more than"), "{}", e.report());
}

#[test]
fn rejects_a_bomb_that_lies_about_its_size() {
    // The directory claims 10 bytes; the data inflates to more. The inflate cap stops it.
    let data = vec![b'a'; 100];
    let mut bytes = zip(&[deflated("a.txt", &data)]);
    let central = bytes.windows(4).rposition(|w| w == b"PK\x01\x02").unwrap();
    bytes
        .get_mut(central + 24..central + 28)
        .unwrap()
        .copy_from_slice(&10u32.to_le_bytes());
    let mut archive = Archive::open(&bytes).unwrap();
    assert!(archive.read("a.txt").is_err());
}

/// Sets the declared compressed and uncompressed sizes of every central directory entry.
fn declare(bytes: &mut [u8], size: u32) {
    let mut at = 0;
    while let Some(i) = bytes
        .get(at..)
        .and_then(|b| b.windows(4).position(|w| w == b"PK\x01\x02"))
    {
        let c = at + i;
        for field in [20, 24] {
            bytes
                .get_mut(c + field..c + field + 4)
                .unwrap()
                .copy_from_slice(&size.to_le_bytes());
        }
        at = c + 4;
    }
}

#[test]
fn rejects_oversize_parts_and_totals() {
    let mut big = zip(&[stored("a.xml", b"x")]);
    declare(&mut big, limits::MAX_PART as u32 + 1);
    assert!(
        Archive::open(&big)
            .unwrap_err()
            .report()
            .contains("over 32 MiB")
    );
    // Five parts that each fit the part limit add up to over the total.
    let entries: Vec<Entry> = (0..5).map(|i| stored(&format!("p{i}.xml"), b"x")).collect();
    let mut total = zip(&entries);
    declare(&mut total, limits::MAX_PART as u32);
    assert!(
        Archive::open(&total)
            .unwrap_err()
            .report()
            .contains("128 MiB")
    );
    declare(&mut total, 1000);
    assert!(Archive::open(&total).is_ok());
}

#[test]
fn rejects_too_many_entries() {
    let entries: Vec<Entry> = (0..=limits::MAX_ENTRIES)
        .map(|i| stored(&format!("p{i}"), b""))
        .collect();
    let bytes = zip(&entries);
    assert!(
        Archive::open(&bytes)
            .unwrap_err()
            .report()
            .contains("entries")
    );
}

#[test]
fn rejects_encryption_zip64_and_other_methods() {
    let base = zip(&[stored("a.txt", b"hello")]);
    let central = base.windows(4).rposition(|w| w == b"PK\x01\x02").unwrap();

    let mut encrypted = base.clone();
    *encrypted.get_mut(central + 8).unwrap() = 1;
    assert!(
        Archive::open(&encrypted)
            .unwrap_err()
            .report()
            .contains("encrypted")
    );

    let mut method = base.clone();
    *method.get_mut(central + 10).unwrap() = 12;
    // An unsupported method fails only the entry that uses it.
    assert!(
        Archive::open(&method)
            .unwrap()
            .read("a.txt")
            .unwrap_err()
            .report()
            .contains("method")
    );

    let mut z64 = base.clone();
    z64.get_mut(central + 20..central + 24)
        .unwrap()
        .copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Archive::open(&z64).unwrap_err().report().contains("zip64"));

    let mut disk = base;
    let eocd = disk.windows(4).rposition(|w| w == b"PK\x05\x06").unwrap();
    *disk.get_mut(eocd + 4).unwrap() = 1;
    assert!(
        Archive::open(&disk)
            .unwrap_err()
            .report()
            .contains("multi-disk")
    );
}

#[test]
fn rejects_a_damaged_local_header() {
    let mut bytes = zip(&[stored("a.txt", b"hello")]);
    *bytes.get_mut(0).unwrap() = b'X';
    // The magic check fails first for a package; a mid-archive local header is read on demand.
    assert!(Archive::open(&bytes).is_err());
    let mut bytes = zip(&[stored("a.txt", b"hello"), stored("b.txt", b"x")]);
    let second = bytes.windows(4).rposition(|w| w == b"PK\x03\x04").unwrap();
    *bytes.get_mut(second + 1).unwrap() = 0;
    let mut archive = Archive::open(&bytes).unwrap();
    assert!(archive.read("a.txt").is_ok());
    assert!(
        archive
            .read("b.txt")
            .unwrap_err()
            .report()
            .contains("damaged header")
    );
}
