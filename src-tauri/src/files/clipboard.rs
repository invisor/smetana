//! The system clipboard, for **files** rather than for text.
//!
//! `tauri-plugin-clipboard-manager` is in the tree and stays exactly what it
//! is: the thing that puts a path on the clipboard as **text**, for the two
//! "Copy path" rows of the file tree's menu. It has no notion of a file list at
//! all, on any platform, so there was no capability to widen and no feature to
//! turn on — which is the whole reason this module exists rather than a line in
//! `capabilities/default.json`.
//!
//! **Three formats, one per platform, and they are not interchangeable.**
//!
//! - **macOS** — `NSPasteboard` with `public.file-url`, one URL per pasteboard
//!   item. There is no cut for files in Finder at all: the move is decided at
//!   *paste* time with Cmd+Opt+V, so nothing here writes a mode and a read
//!   always answers `copy`. Writing one would be inventing a field Finder never
//!   reads and never sets.
//! - **Windows** — `CF_HDROP` for the paths, plus the registered `Preferred
//!   DropEffect` format carrying `DROPEFFECT_MOVE` for a cut and
//!   `DROPEFFECT_COPY` for a copy, which is what Explorer reads and writes.
//! - **Linux** — the GTK clipboard: `text/uri-list` for the paths and
//!   `x-special/gnome-copied-files` (`copy\n<uri>…` / `cut\n<uri>…`) for the
//!   mode. That second one is not a GNOME thing despite the name — Nautilus,
//!   Nemo, Thunar and Dolphin all read and write it.
//!
//! **A clipboard that will not answer is an ordinary outcome and never fatal.**
//! A read that fails answers an empty list and the paste rides on the tree's own
//! record; a write that fails does not fail the copy that asked for it. That is
//! the same standing `list_dir` takes towards a folder outside git, and the
//! front end's half of it is in `stores/files.js`.
//!
//! **On Linux every call here has to be made on the main thread**, because GTK
//! says so: the clipboard is a GTK object bound to the display connection the
//! window loop owns, and reaching it from the blocking pool is undefined at
//! best. `dispatch` below is where that happens — `AppHandle::run_on_main_thread`
//! with the answer coming back over a channel — and it is why both commands
//! take an `AppHandle` they use on exactly one of the three platforms. macOS's
//! `NSPasteboard` is documented thread-safe and Windows' clipboard is per
//! process rather than per thread, so neither needs it.

use serde::Serialize;

use super::model::FilesError;

/// What the system clipboard holds, in the front end's terms: absolute paths
/// and one word for the mode.
///
/// Absolute in both directions, which is not the tree's own spelling and is
/// deliberate: a path on the system clipboard was put there by Finder or by
/// Explorer or by this app, and it names a place on the disk rather than a row
/// in a project. Turning one into a tree path — or finding that it is not in
/// this project at all, which is an ordinary case — is the front end's, where
/// the project root is known.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardFiles {
    pub paths: Vec<String>,
    pub mode: String,
}

/// The one word for the mode, and the only two values that cross the IPC.
///
/// Anything that is not `cut` is a copy. A clipboard read on a platform that
/// states no mode answers `copy`, and so does an unknown word from a front end
/// a version ahead of this one: of the two ways to be wrong, copying something
/// that was meant to move leaves both files, and moving something that was
/// meant to be copied takes one away.
fn normalize_mode(mode: &str) -> &'static str {
    if mode == "cut" {
        "cut"
    } else {
        "copy"
    }
}

/// The two names the GTK clipboard knows a file list by. Here rather than in
/// the Linux module so the rule that formats them can be tested on any platform
/// — see below.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const GNOME_COPIED_FILES: &str = "x-special/gnome-copied-files";
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const URI_LIST: &str = "text/uri-list";

/// The `x-special/gnome-copied-files` payload: the verb, then one URI per line.
///
/// It is the only place the mode is written on Linux, and the separator is a
/// bare newline rather than the CRLF `text/uri-list` wants — the two targets
/// are different formats that happen to carry the same URIs, and a file manager
/// reading this one splits on `\n`.
///
/// Not behind the Linux `cfg`, deliberately, and it is the only part of that
/// half of this module a test on this machine can reach: the GTK calls need a
/// display, a main loop and a Linux build, while this is a string. A rule
/// nothing can check is a rule that drifts.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn gnome_copied_files(mode: &str, uris: &[String]) -> String {
    let mut out = String::from(normalize_mode(mode));
    for uri in uris {
        out.push('\n');
        out.push_str(uri);
    }
    out
}

/// The same payload read back: the verb and the URIs under it.
///
/// A payload that does not open with a verb is not refused — what follows is
/// read as URIs and `copy` is assumed, which is the standing the whole module
/// takes about a mode nobody stated. Empty lines are dropped, since a trailing
/// newline is ordinary and an empty URI is not a path.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_gnome_copied_files(text: &str) -> (&'static str, Vec<String>) {
    let mut lines = text.split('\n').map(str::trim).filter(|line| !line.is_empty()).peekable();
    let mode = match lines.peek() {
        Some(&"cut") => {
            lines.next();
            "cut"
        }
        Some(&"copy") => {
            lines.next();
            "copy"
        }
        _ => "copy",
    };
    (mode, lines.map(str::to_owned).collect())
}

/// How long the main thread is given to come back with an answer, and the one
/// ceiling in this module.
///
/// It is here because the GTK call underneath cannot be bounded from Rust at
/// all: `gtk_clipboard_wait_for_contents` runs a nested main loop until the
/// clipboard's *owner* — another program entirely — answers, and there is no
/// timeout to pass it. What can be bounded is this side of the channel, and
/// that is worth doing on its own: without it a clipboard owner that never
/// answers parks a blocking-pool worker for the life of the process, and this
/// is asked on every window focus and every context menu.
///
/// Half a second, because of when the answer is wanted rather than because of
/// what the call costs. It is read while a menu is opening; a reply that
/// arrives later than that has missed the panel it was for, and the row is
/// drawn from the last answer either way. An ordinary local clipboard replies
/// in single-digit milliseconds.
#[cfg(target_os = "linux")]
const MAIN_THREAD_BUDGET: std::time::Duration = std::time::Duration::from_millis(500);

/// Where the work is done, and the one thing that differs by platform besides
/// the format itself.
///
/// On Linux it is the main thread, because GTK's clipboard belongs to the
/// window loop and nothing else may touch it; the caller is already on the
/// blocking pool (`off_the_runtime` in `commands.rs`), so blocking that thread
/// on the answer parks nothing the app needs. Everywhere else the call is made
/// where it stands.
#[cfg(target_os = "linux")]
fn dispatch<T, F>(app: &tauri::AppHandle, work: F) -> Result<T, FilesError>
where
    F: FnOnce() -> Result<T, FilesError> + Send + 'static,
    T: Send + 'static,
{
    let (answer, wait) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        // Nothing to do about a receiver that has gone: the caller stopped
        // waiting, and the clipboard call has already happened either way.
        let _ = answer.send(work());
    })
    .map_err(|err| FilesError::Io(format!("the clipboard could not be reached: {err}")))?;
    // Giving up on the answer is not giving up on the work: the closure is the
    // main thread's now and runs whenever it gets there. What is bought is this
    // thread, and the caller reads the refusal as "no files", which is what
    // every other failure here reads as.
    wait.recv_timeout(MAIN_THREAD_BUDGET)
        .map_err(|_| FilesError::Io("the clipboard did not answer in time".to_owned()))?
}

#[cfg(not(target_os = "linux"))]
fn dispatch<T, F>(app: &tauri::AppHandle, work: F) -> Result<T, FilesError>
where
    F: FnOnce() -> Result<T, FilesError> + Send + 'static,
    T: Send + 'static,
{
    let _ = app;
    work()
}

/// The paths onto the system clipboard, and the mode with them where the
/// platform has somewhere to put one.
pub fn write_files(
    app: &tauri::AppHandle,
    paths: &[String],
    mode: &str,
) -> Result<(), FilesError> {
    let paths = paths.to_vec();
    let mode = normalize_mode(mode);
    dispatch(app, move || platform::write(&paths, mode))
}

/// What is on the system clipboard now, or nothing at all — see the header:
/// this is a question the answer to which is allowed to be "no files here".
pub fn read_files(app: &tauri::AppHandle) -> Result<ClipboardFiles, FilesError> {
    dispatch(app, platform::read)
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardWriting};
    use objc2_foundation::{NSArray, NSString, NSURL};

    use super::{ClipboardFiles, FilesError};

    /// One `NSURL` per pasteboard item, which is the shape Finder writes and the
    /// only shape it reads: a single item holding several URLs is a file list to
    /// nothing at all.
    ///
    /// `mode` is taken and dropped on purpose. Finder has no cut for files —
    /// the move is Cmd+Opt+V at paste time and lives nowhere on the pasteboard
    /// — so a mode written here would be a field this app is the only reader
    /// of, and the tree already has one of those: its own record.
    pub fn write(paths: &[String], mode: &str) -> Result<(), FilesError> {
        let _ = mode;
        let pasteboard = NSPasteboard::generalPasteboard();
        // Clearing is what takes ownership; the write is refused without it.
        pasteboard.clearContents();
        if paths.is_empty() {
            return Ok(());
        }
        let items: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = paths
            .iter()
            .map(|path| {
                ProtocolObject::from_retained(NSURL::fileURLWithPath(&NSString::from_str(path)))
            })
            .collect();
        if !pasteboard.writeObjects(&NSArray::from_retained_slice(&items)) {
            return Err(FilesError::Io("the pasteboard refused the file list".to_owned()));
        }
        Ok(())
    }

    /// Every item carrying a `public.file-url`, in the order the pasteboard
    /// holds them. An item that carries something else — text, an image, a
    /// promise of a file not written yet — is passed over rather than refused:
    /// a clipboard of mixed contents is ordinary, and the answer to "what files
    /// are on it" is the files.
    pub fn read() -> Result<ClipboardFiles, FilesError> {
        let pasteboard = NSPasteboard::generalPasteboard();
        let mut paths = Vec::new();
        if let Some(items) = pasteboard.pasteboardItems() {
            for item in items.iter() {
                // SAFETY: an AppKit constant, read and never written.
                let file_url = unsafe { NSPasteboardTypeFileURL };
                let Some(text) = item.stringForType(file_url) else { continue };
                // The item holds the URL as text, so it comes back through
                // `NSURL` rather than being unescaped by hand: `%20` in a path
                // with a space in it is the ordinary case, not the exotic one.
                let Some(url) = NSURL::URLWithString(&text) else { continue };
                // An item can carry `public.file-url` and hold something that
                // is not one: `NSURL` parses `https://example.com/foo` happily
                // and answers a `path` of `/foo`, and a relative string parses
                // to a relative path. Either would travel on to
                // `files_copy_external` and come back as a refusal a person
                // sees, over a clipboard they had no reason to think was
                // broken. Passed over instead, which is what the paragraph
                // above promises about an item carrying something else.
                if !url.isFileURL() {
                    continue;
                }
                let Some(path) = url.path() else { continue };
                paths.push(path.to_string());
            }
        }
        Ok(ClipboardFiles { paths, mode: "copy".to_owned() })
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::ffi::{OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    use windows::core::w;
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, RegisterClipboardFormatW,
        SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows::Win32::System::Ole::{CF_HDROP, DROPEFFECT_COPY, DROPEFFECT_MOVE};
    use windows::Win32::UI::Shell::{DragQueryFileW, DROPFILES, HDROP};

    use super::{ClipboardFiles, FilesError};

    /// The clipboard is a process-wide lock, and a process that opens it and
    /// does not close it locks it for every other program on the machine until
    /// it exits. So it is opened through a guard: whatever goes wrong in
    /// between, the close happens on the way out.
    struct Clipboard;

    impl Clipboard {
        fn open() -> Result<Self, FilesError> {
            // `None` for the owner window: this process owns it, and there is
            // no HWND to name that would survive the webview being recreated.
            unsafe { OpenClipboard(None) }
                .map_err(|err| FilesError::Io(format!("the clipboard would not open: {err}")))?;
            Ok(Self)
        }
    }

    impl Drop for Clipboard {
        fn drop(&mut self) {
            let _ = unsafe { CloseClipboard() };
        }
    }

    /// `CF_HDROP`'s payload: a `DROPFILES` header, then the paths as wide
    /// strings one after another, each terminated by a NUL and the whole list
    /// by a second one.
    ///
    /// `fWide` is what says the names are UTF-16 rather than the ANSI code
    /// page, and `pFiles` is the offset the names start at — the size of the
    /// header, since they follow it immediately.
    fn hdrop_block(paths: &[String]) -> Vec<u8> {
        let mut names: Vec<u16> = Vec::new();
        for path in paths {
            names.extend(OsStr::new(path).encode_wide());
            names.push(0);
        }
        names.push(0);
        let header = DROPFILES {
            pFiles: std::mem::size_of::<DROPFILES>() as u32,
            pt: Default::default(),
            fNC: false.into(),
            fWide: true.into(),
        };
        let mut block = Vec::with_capacity(std::mem::size_of::<DROPFILES>() + names.len() * 2);
        // SAFETY: `DROPFILES` is `repr(C, packed(1))` over integers alone —
        // `POINT` is two `i32` and `BOOL` is a newtype over one — so it holds
        // no padding and no pointer, and its bytes are exactly what the shell
        // expects to read back.
        block.extend_from_slice(unsafe {
            std::slice::from_raw_parts(
                std::ptr::addr_of!(header).cast::<u8>(),
                std::mem::size_of::<DROPFILES>(),
            )
        });
        for unit in names {
            block.extend_from_slice(&unit.to_le_bytes());
        }
        block
    }

    /// A block of global memory **this process still owns**, and a guard for the
    /// one moment it stops owning it.
    ///
    /// `SetClipboardData` takes the block over on success and only then. Every
    /// path before that point — a lock that failed, a clipboard that refused —
    /// leaves the memory ours, and memory nobody frees is leaked for the life
    /// of the process. So the block is a value with a `Drop`, and `given_away`
    /// is the one call that stops the free from happening.
    struct Block(HGLOBAL);

    impl Block {
        /// What `SetClipboardData` takes: the same pointer, in the type that
        /// call is written in.
        fn handle(&self) -> HANDLE {
            HANDLE(self.0 .0)
        }

        /// Ownership has passed to the clipboard, so the drop must not run.
        fn given_away(self) {
            std::mem::forget(self);
        }
    }

    impl Drop for Block {
        fn drop(&mut self) {
            let _ = unsafe { GlobalFree(Some(self.0)) };
        }
    }

    /// One block of bytes onto the clipboard under one format.
    fn put(format: u32, bytes: &[u8]) -> Result<(), FilesError> {
        let block = Block(unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }.map_err(|err| {
            FilesError::Io(format!("the clipboard block could not be allocated: {err}"))
        })?);
        let target = unsafe { GlobalLock(block.0) };
        if target.is_null() {
            return Err(FilesError::Io("the clipboard block could not be locked".to_owned()));
        }
        // SAFETY: the block was just allocated at `bytes.len()` and is locked.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), target.cast::<u8>(), bytes.len()) };
        // Answers `Err` when the lock count reaches zero, which is this call.
        let _ = unsafe { GlobalUnlock(block.0) };
        match unsafe { SetClipboardData(format, Some(block.handle())) } {
            Ok(_) => {
                block.given_away();
                Ok(())
            }
            Err(err) => Err(FilesError::Io(format!("the clipboard refused the block: {err}"))),
        }
    }

    /// The format Explorer states a cut in. It is registered rather than
    /// standard, so the number is whatever this session's first caller got, and
    /// asking for it again answers the same number.
    fn drop_effect_format() -> u32 {
        unsafe { RegisterClipboardFormatW(w!("Preferred DropEffect")) }
    }

    pub fn write(paths: &[String], mode: &str) -> Result<(), FilesError> {
        let _guard = Clipboard::open()?;
        unsafe { EmptyClipboard() }
            .map_err(|err| FilesError::Io(format!("the clipboard would not empty: {err}")))?;
        if paths.is_empty() {
            return Ok(());
        }
        put(CF_HDROP.0 as u32, &hdrop_block(paths))?;
        let effect = if mode == "cut" { DROPEFFECT_MOVE } else { DROPEFFECT_COPY };
        let format = drop_effect_format();
        // A registration that failed is a mode nobody will read, and a paste
        // that copies where it should have moved is a smaller loss than a
        // refused copy: the paths are already on the clipboard.
        if format != 0 {
            put(format, &effect.0.to_le_bytes())?;
        }
        Ok(())
    }

    pub fn read() -> Result<ClipboardFiles, FilesError> {
        let _guard = Clipboard::open()?;
        let mut paths = Vec::new();
        // A clipboard holding no file list at all is an ordinary answer, so the
        // refusal here is dropped rather than reported.
        if let Ok(handle) = unsafe { GetClipboardData(CF_HDROP.0 as u32) } {
            let names = HDROP(handle.0);
            // `u32::MAX` as the index is how the shell is asked for the count.
            let count = unsafe { DragQueryFileW(names, u32::MAX, None) };
            for index in 0..count {
                let len = unsafe { DragQueryFileW(names, index, None) };
                if len == 0 {
                    continue;
                }
                // One more for the NUL the shell writes and the length above
                // does not count.
                let mut buffer = vec![0u16; len as usize + 1];
                let written = unsafe { DragQueryFileW(names, index, Some(&mut buffer)) };
                if written == 0 {
                    continue;
                }
                paths.push(
                    OsString::from_wide(&buffer[..written as usize]).to_string_lossy().into_owned(),
                );
            }
        }
        Ok(ClipboardFiles { paths, mode: read_mode().to_owned() })
    }

    /// What Explorer said it meant, or `copy` where it said nothing. Every
    /// refusal on the way is that same answer: a mode is an extra, and the
    /// paths are the point.
    fn read_mode() -> &'static str {
        let format = drop_effect_format();
        if format == 0 {
            return "copy";
        }
        let Ok(handle) = (unsafe { GetClipboardData(format) }) else { return "copy" };
        let block = HGLOBAL(handle.0);
        if unsafe { GlobalSize(block) } < 4 {
            return "copy";
        }
        let locked = unsafe { GlobalLock(block) };
        if locked.is_null() {
            return "copy";
        }
        let mut value = [0u8; 4];
        // SAFETY: the block is locked and was just measured at four bytes or
        // more, which is the DWORD this format is.
        unsafe { std::ptr::copy_nonoverlapping(locked.cast::<u8>(), value.as_mut_ptr(), 4) };
        let _ = unsafe { GlobalUnlock(block) };
        // A mask rather than an equality: Explorer sets `DROPEFFECT_LINK`
        // alongside the others often enough that a match on the whole word
        // would read a cut as a copy.
        if u32::from_le_bytes(value) & DROPEFFECT_MOVE.0 != 0 {
            "cut"
        } else {
            "copy"
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use gtk::gdk;
    use gtk::glib;

    use super::{
        gnome_copied_files, parse_gnome_copied_files, ClipboardFiles, FilesError,
        GNOME_COPIED_FILES, URI_LIST,
    };

    /// Two targets served from one closure, told apart by the `info` number the
    /// entries carry. The paths are the same in both; only the framing differs.
    ///
    /// **What this does not do is survive the app**, and it is worth saying out
    /// loud rather than leaving to be discovered: an X11 or Wayland clipboard is
    /// served by the process that owns it, so the file list is available for as
    /// long as this app is running and no longer. Handing it to a clipboard
    /// manager on the way out would be `gtk_clipboard_set_can_store`, which
    /// gtk-rs does not bind, so `store` alone would store nothing.
    pub fn write(paths: &[String], mode: &str) -> Result<(), FilesError> {
        let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
        if paths.is_empty() {
            clipboard.clear();
            return Ok(());
        }
        let uris = paths
            .iter()
            .map(|path| glib::filename_to_uri(path, None).map(|uri| uri.to_string()))
            .collect::<Result<Vec<String>, _>>()
            .map_err(|err| FilesError::Io(format!("a path is not a URI: {err}")))?;
        // CRLF, which is what RFC 2483 says `text/uri-list` is, including after
        // the last one. The GNOME target below is newline-separated instead —
        // two formats that happen to carry the same URIs.
        let uri_list = uris.iter().map(|uri| format!("{uri}\r\n")).collect::<String>();
        let with_mode = gnome_copied_files(mode, &uris);
        let targets = [
            gtk::TargetEntry::new(GNOME_COPIED_FILES, gtk::TargetFlags::empty(), 0),
            gtk::TargetEntry::new(URI_LIST, gtk::TargetFlags::empty(), 1),
        ];
        let served = clipboard.set_with_data(&targets, move |_, selection, info| {
            let bytes =
                if info == 0 { with_mode.as_bytes() } else { uri_list.as_bytes() };
            // Eight bits per unit: these are bytes and not a list of integers,
            // which is what every reader of both formats expects.
            selection.set(&selection.target(), 8, bytes);
        });
        if !served {
            return Err(FilesError::Io("the clipboard refused the file list".to_owned()));
        }
        Ok(())
    }

    /// The mode-carrying target first, and `text/uri-list` behind it. A file
    /// manager that wrote only the second is an ordinary case — that is what a
    /// browser puts there, among others — and it means a copy.
    pub fn read() -> Result<ClipboardFiles, FilesError> {
        let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
        if let Some(data) = clipboard.wait_for_contents(&gdk::Atom::intern(GNOME_COPIED_FILES)) {
            let bytes = data.data();
            let (mode, uris) = parse_gnome_copied_files(&String::from_utf8_lossy(&bytes));
            let paths = paths_of(&uris);
            if !paths.is_empty() {
                return Ok(ClipboardFiles { paths, mode: mode.to_owned() });
            }
        }
        let uris: Vec<String> =
            clipboard.wait_for_uris().iter().map(|uri| uri.to_string()).collect();
        Ok(ClipboardFiles { paths: paths_of(&uris), mode: "copy".to_owned() })
    }

    /// A URI that is not a local file — `http:`, `trash:`, a URI from a device
    /// that is not mounted — has no path and is dropped. Copying it would mean
    /// fetching something, which a paste in a file tree does not mean.
    fn paths_of(uris: &[String]) -> Vec<String> {
        uris.iter()
            .filter_map(|uri| glib::filename_from_uri(uri).ok())
            .map(|(path, _host)| path.to_string_lossy().into_owned())
            .collect()
    }
}

/// Every other platform, which this app is not built for and which this module
/// declines to pretend about: no clipboard, no files, and a write that goes
/// nowhere rather than a compile error in a file nobody is looking at.
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod platform {
    use super::{ClipboardFiles, FilesError};

    pub fn write(paths: &[String], mode: &str) -> Result<(), FilesError> {
        let _ = (paths, mode);
        Ok(())
    }

    pub fn read() -> Result<ClipboardFiles, FilesError> {
        Ok(ClipboardFiles { paths: Vec::new(), mode: "copy".to_owned() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mode_that_is_not_cut_is_a_copy() {
        assert_eq!(normalize_mode("cut"), "cut");
        assert_eq!(normalize_mode("copy"), "copy");
        assert_eq!(normalize_mode(""), "copy");
        assert_eq!(normalize_mode("move"), "copy");
    }

    #[test]
    fn the_gnome_payload_is_the_verb_and_then_one_uri_per_line() {
        let uris = vec!["file:///p/a.txt".to_owned(), "file:///p/b.txt".to_owned()];
        assert_eq!(
            gnome_copied_files("cut", &uris),
            "cut\nfile:///p/a.txt\nfile:///p/b.txt"
        );
        assert_eq!(
            gnome_copied_files("copy", &uris),
            "copy\nfile:///p/a.txt\nfile:///p/b.txt"
        );
    }

    #[test]
    fn a_mode_no_platform_states_is_written_as_a_copy() {
        // The same normalization the IPC boundary makes, so a payload cannot
        // carry a word a file manager has never heard of.
        assert_eq!(gnome_copied_files("move", &["file:///p/a".to_owned()]), "copy\nfile:///p/a");
    }

    #[test]
    fn the_gnome_payload_reads_back_as_it_was_written() {
        let uris = vec!["file:///p/a.txt".to_owned(), "file:///p/b.txt".to_owned()];
        let (mode, back) = parse_gnome_copied_files(&gnome_copied_files("cut", &uris));
        assert_eq!(mode, "cut");
        assert_eq!(back, uris);
    }

    #[test]
    fn a_trailing_newline_is_not_a_uri() {
        let (mode, uris) = parse_gnome_copied_files("copy\nfile:///p/a.txt\n");
        assert_eq!(mode, "copy");
        assert_eq!(uris, ["file:///p/a.txt"]);
    }

    #[test]
    fn a_payload_that_states_no_verb_is_read_as_a_copy_and_keeps_every_line() {
        let (mode, uris) = parse_gnome_copied_files("file:///p/a.txt\nfile:///p/b.txt");
        assert_eq!(mode, "copy");
        assert_eq!(uris, ["file:///p/a.txt", "file:///p/b.txt"]);
    }

    #[test]
    fn an_empty_payload_is_no_files_rather_than_a_refusal() {
        let (mode, uris) = parse_gnome_copied_files("");
        assert_eq!(mode, "copy");
        assert!(uris.is_empty());
    }
}
