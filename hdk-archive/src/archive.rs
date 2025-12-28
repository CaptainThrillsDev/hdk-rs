use std::io::Read;
use std::ops::Range;

/// Bundles copyable entry metadata plus a reader for the entry content.
///
/// The reader lifetime is tied to the archive reader borrow when the entry
/// content is streamed from the underlying archive.
pub struct EntryStream<'a, M> {
    pub metadata: M,
    pub reader: Box<dyn Read + 'a>,
}

/// Common streaming read API shared by archive readers (e.g. BAR, SHARC).
///
/// This trait intentionally does **not** include construction/opening, because
/// formats may require different parameters (e.g. SHARC keys).
pub trait ArchiveReader {
    type Metadata: Copy;

    fn is_empty(&self) -> bool {
        self.entry_count() == 0
    }

    fn entry_count(&self) -> usize;

    fn entry_metadata(&self, index: usize) -> std::io::Result<Self::Metadata>;

    /// Iterate copyable metadata for all entries.
    fn entries(&self) -> impl Iterator<Item = std::io::Result<Self::Metadata>> + '_ {
        self.entry_indices().map(|i| self.entry_metadata(i))
    }

    /// Iterate entry indices (`0..len`).
    fn entry_indices(&self) -> Range<usize> {
        0..self.entry_count()
    }

    /// Stream an entry's content.
    ///
    /// This borrows `self` mutably because implementations typically `seek()` the
    /// underlying reader.
    fn entry_reader<'a>(&'a mut self, index: usize) -> std::io::Result<Box<dyn Read + 'a>>;

    fn entry<'a>(&'a mut self, index: usize) -> std::io::Result<EntryStream<'a, Self::Metadata>> {
        let metadata = self.entry_metadata(index)?;
        let reader = self.entry_reader(index)?;
        Ok(EntryStream { metadata, reader })
    }

    /// Visit each entry sequentially, yielding metadata + a streaming reader.
    ///
    /// This is the ergonomic alternative to trying to build a true
    /// `Iterator<Item = EntryStream<...>>`, which doesn't work well with Rust's
    /// borrowing rules because each yielded reader borrows `&mut self`.
    fn for_each_entry<F>(&mut self, mut f: F) -> std::io::Result<()>
    where
        F: for<'a> FnMut(EntryStream<'a, Self::Metadata>) -> std::io::Result<()>,
    {
        for i in self.entry_indices() {
            let entry = self.entry(i)?;
            f(entry)?;
        }
        Ok(())
    }
}
