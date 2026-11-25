//! The bounds on a `.vsdx` package and its drawing (VISIO-13, as LANGS-8 bounds every language).

/// The archive is at most 64 MiB.
pub(crate) const MAX_ARCHIVE: usize = 64 * 1024 * 1024;

/// The archive has at most this many entries.
pub(crate) const MAX_ENTRIES: usize = 4096;

/// One part is at most 32 MiB uncompressed.
pub(crate) const MAX_PART: usize = 32 * 1024 * 1024;

/// The parts of an archive add up to at most 128 MiB uncompressed (declared and read).
pub(crate) const MAX_TOTAL: usize = 128 * 1024 * 1024;

/// An entry over [`RATIO_FLOOR`] bytes uncompressed is at most this many times its compressed size.
pub(crate) const MAX_RATIO: usize = 200;

/// Entries this small or smaller are not checked against [`MAX_RATIO`].
pub(crate) const RATIO_FLOOR: usize = 1024;

/// An XML part is at most 16 MiB.
pub(crate) const MAX_XML: usize = 16 * 1024 * 1024;

/// An XML part has at most this many nodes.
pub(crate) const MAX_XML_NODES: u32 = 1_000_000;

/// XML elements nest at most this deep.
pub(crate) const MAX_XML_DEPTH: usize = 256;

/// A document has at most this many pages.
pub(crate) const MAX_PAGES: usize = 256;

/// A document has at most this many masters.
pub(crate) const MAX_MASTERS: usize = 4096;

/// A page draws at most this many shapes, each master instance counted by the shapes it expands to.
pub(crate) const MAX_SHAPES: usize = 50_000;

/// A page draws at most this many geometry rows over all its shapes, inherited rows counted once
/// for each shape that inherits them.
pub(crate) const MAX_PAGE_ROWS: usize = 1_000_000;

/// A page lays out at most this many bytes of text over all its shapes, inherited text counted
/// once for each shape that inherits it.
pub(crate) const MAX_PAGE_TEXT: usize = 4 * 1024 * 1024;

/// Groups nest at most this deep.
pub(crate) const MAX_GROUP_DEPTH: usize = 64;

/// A shape has at most this many geometry rows, over all its geometry sections.
pub(crate) const MAX_ROWS: usize = 4096;

/// A cell is looked up through at most this many masters and styles.
pub(crate) const MAX_STYLE_DEPTH: usize = 16;

/// A shape's text is at most 64 KiB.
pub(crate) const MAX_TEXT: usize = 64 * 1024;

/// A page is at most this many inches on a side (a drawing of 100 inches is 9,600 px).
pub(crate) const MAX_PAGE_INCHES: f64 = 200.0;

/// A cell value of a transform is at most this many inches (or radians) in magnitude.
pub(crate) const MAX_COORD: f64 = 1e6;

/// Points read from one `POLYLINE` or `NURBS` formula.
pub(crate) const MAX_FORMULA_POINTS: usize = 4096;
