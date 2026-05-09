use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{Datelike, Local};
use typst::{
    diag::{FileError, FileResult, SourceDiagnostic},
    foundations::{Bytes, Datetime},
    layout::PagedDocument,
    syntax::{FileId, Source, VirtualPath},
    text::{Font, FontBook},
    utils::LazyHash,
    Library, LibraryExt, World,
};
use typst_kit::fonts::{FontSearcher, FontSlot, Fonts};

/// Pre-built renderer state. Fonts and the standard library are loaded once at
/// startup and shared across renders.
pub struct PdfRenderer {
    library: Arc<LazyHash<Library>>,
    book: Arc<LazyHash<FontBook>>,
    fonts: Arc<Vec<FontSlot>>,
    root: PathBuf,
}

impl PdfRenderer {
    /// Discover system + embedded fonts and build the renderer. `root` is the
    /// project root that absolute paths in the Typst source resolve against
    /// (e.g. `image("/templates/foo.svg")` -> `<root>/templates/foo.svg`).
    pub fn new(root: PathBuf) -> Self {
        let Fonts { book, fonts } = FontSearcher::new()
            .include_system_fonts(true)
            .search();
        Self {
            library: Arc::new(LazyHash::new(Library::default())),
            book: Arc::new(LazyHash::new(book)),
            fonts: Arc::new(fonts),
            root,
        }
    }

    /// Compile `source` (Typst markup) into a PDF.
    pub fn render(&self, source: String) -> anyhow::Result<Vec<u8>> {
        let main_id = FileId::new(None, VirtualPath::new("/main.typ"));
        let main = Source::new(main_id, source);
        let world = PdfWorld {
            library: self.library.clone(),
            book: self.book.clone(),
            fonts: self.fonts.clone(),
            root: self.root.clone(),
            main,
        };
        let warned = typst::compile::<PagedDocument>(&world);
        let document = warned
            .output
            .map_err(|errs| format_diagnostics("compile", &errs))?;
        let bytes = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
            .map_err(|errs| format_diagnostics("pdf export", &errs))?;
        Ok(bytes)
    }
}

fn format_diagnostics(stage: &str, errs: &[SourceDiagnostic]) -> anyhow::Error {
    let mut s = String::new();
    for e in errs {
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(&e.message);
        for h in &e.hints {
            s.push_str("\n  hint: ");
            s.push_str(h);
        }
    }
    anyhow::anyhow!("typst {stage}: {s}")
}

struct PdfWorld {
    library: Arc<LazyHash<Library>>,
    book: Arc<LazyHash<FontBook>>,
    fonts: Arc<Vec<FontSlot>>,
    root: PathBuf,
    main: Source,
}

impl World for PdfWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }
    fn main(&self) -> FileId {
        self.main.id()
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            return Ok(self.main.clone());
        }
        let path = self.resolve(id)?;
        let text =
            std::fs::read_to_string(&path).map_err(|err| FileError::from_io(err, &path))?;
        Ok(Source::new(id, text))
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = self.resolve(id)?;
        let bytes = std::fs::read(&path).map_err(|err| FileError::from_io(err, &path))?;
        Ok(Bytes::new(bytes))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index)?.get()
    }
    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        let now = Local::now();
        Datetime::from_ymd(now.year(), now.month() as u8, now.day() as u8)
    }
}

impl PdfWorld {
    fn resolve(&self, id: FileId) -> FileResult<PathBuf> {
        if id.package().is_some() {
            return Err(FileError::Other(Some(
                "remote packages not supported".into(),
            )));
        }
        id.vpath()
            .resolve(&self.root)
            .ok_or(FileError::AccessDenied)
            .and_then(|p| {
                if path_within(&p, &self.root) {
                    Ok(p)
                } else {
                    Err(FileError::AccessDenied)
                }
            })
    }
}

fn path_within(path: &Path, root: &Path) -> bool {
    let canon = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let canon_root = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    canon.starts_with(canon_root)
}
