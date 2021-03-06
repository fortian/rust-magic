//! # About
//!
//! This crate provides bindings for `libmagic`, which recognizes the
//! type of data contained in a file (or buffer).
//!
//! You might be similar with `libmagic`'s CLI `file`:
//!
//! ```sh
//! $ file data/tests/rust-logo-128x128-blk.png
//! data/tests/rust-logo-128x128-blk.png: PNG image data, 128 x 128, 8-bit colormap, non-interlaced
//! ```
//!
//! # Usage example
//!
//! Here's an example of using this crate:
//!
//! ```
//! extern crate magic;
//! use magic::{Magic, MagicFlags};
//!
//! fn main() {
//!     // Create a new default configuration and load one specific magic
//!     // database.
//!     let databases = vec!["data/tests/db-images-png"];
//!     let magic = Magic::new(MagicFlags::default(), &databases).unwrap();
//!
//!     // Recognize the magic of a test file
//!     let test_file_path = "data/tests/rust-logo-128x128-blk.png";
//!     let expected_magic = "PNG image data, 128 x 128, 8-bit colormap, non-interlaced";
//!     assert_eq!(magic.file(&test_file_path).unwrap(), expected_magic);
//! }
//! ```

extern crate errno;
extern crate libc;
extern crate magic_sys as ffi;
#[macro_use]
extern crate bitflags;

use errno::errno;
use libc::size_t;
use std::error;
use std::ffi::{CStr, CString};
use std::fmt::Display;
use std::ptr;
use std::str;

macro_rules! from_c_str_unsafe {
    ($x:expr) => {
        unsafe { CStr::from_ptr($x).to_string_lossy().into_owned() }
    };
}

// Make it easier to use `MagicFlags::default()` and such
pub use flags::MagicFlags;

/// Bitmask flags which control `libmagic` behaviour
pub mod flags {
    use libc::c_int;

    bitflags! {
        /// Bitmask flags that specify how `Magic` functions should behave
        /// 
        /// These descriptions are taken from `man -s 3 libmagic` (with some
        /// slight corrections).
        pub struct MagicFlags: c_int {
            /// No special handling.
            const NONE = 0x000000;

            /// Print debugging messages to `stderr`.
            /// 
            /// These messages are printed by `libmagic` itself, not this crate.
            const DEBUG = 0x000001;

            /// If the file is a symlink, follow it.
            const SYMLINK = 0x000002;

            /// If the file is compressed, unpack it and look at the contents.
            const COMPRESS = 0x000004;

            /// If the file is a block or character special device, then open
            /// the device and try to look in its contents.
            const DEVICES = 0x000008;

            /// Return a MIME type string, instead of a textual description.
            const MIME_TYPE = 0x000010;

            /// Return all matches, not just the first.
            const CONTINUE = 0x000020;

            /// Check the magic database for consistency and print warnings to
            /// `stderr`.
            /// 
            /// These warnings are printed by `libmagic` itself, not this crate.
            const CHECK = 0x000040;

            /// On systems that support `utime(2)` or `utimes(2)`, attempt to
            /// preserve the access time of analyzed files.
            const PRESERVE_ATIME = 0x000080;

            /// Don't translate unprintable characters to a `\\ooo` octal
            /// representation.
            const RAW = 0x000100;

            /// Treat operating system errors while trying to open files and
            /// follow symlinks as real errors, instead of printing them in the
            /// magic buffer.
            const ERROR = 0x000200;

            /// Return a MIME encoding, instead of a textual description.
            const MIME_ENCODING = 0x000400;

            /// Shorthand for `MIME_TYPE | MIME_ENCODING`
            const MIME = MagicFlags::MIME_TYPE.bits
                | MagicFlags::MIME_ENCODING.bits;

            /// Return the Apple creator and type.
            const APPLE = 0x000800;

            /// Return a slash-separated list of extensions.
            const EXTENSION = 0x1000000;

            /// Check inside compressed files but don't report compression.
            const COMPRESS_TRANSP = 0x2000000;

            /// Don't give a description, but return the extension, MIME
            /// type/encoding, and Apple creator/type.
            const NODESC = MagicFlags::EXTENSION.bits
                | MagicFlags::MIME.bits
                | MagicFlags::APPLE.bits;

            /// Don't look inside compressed files.
            const NO_CHECK_COMPRESS = 0x001000;

            /// Don't examine tar files.
            const NO_CHECK_TAR = 0x002000;

            /// Don't consult magic databases.
            const NO_CHECK_SOFT = 0x004000;

            /// Check for EMX application type (only on EMX).
            const NO_CHECK_APPTYPE = 0x008000;

            /// Don't print ELF details.
            const NO_CHECK_ELF = 0x010000;

            /// Don't check for various types of text files.
            const NO_CHECK_TEXT = 0x020000;

            /// Don't get extra information on MS Composite Document Files.
            const NO_CHECK_CDF = 0x040000;

            /// Don't look for known tokens inside ASCII files.
            const NO_CHECK_TOKENS = 0x100000;

            /// Don't check text encodings.
            const NO_CHECK_ENCODING = 0x200000;

            /// Don't check for JSON files.
            const NO_CHECK_JSON = 0x0400000;

            /// No built-in file tests; only consult the magic database.
            const NO_CHECK_BUILTIN = MagicFlags::NO_CHECK_COMPRESS.bits
                | MagicFlags::NO_CHECK_TAR.bits
                | MagicFlags::NO_CHECK_APPTYPE.bits
                | MagicFlags::NO_CHECK_ELF.bits
                | MagicFlags::NO_CHECK_TEXT.bits
                | MagicFlags::NO_CHECK_CDF.bits
                | MagicFlags::NO_CHECK_TOKENS.bits
                | MagicFlags::NO_CHECK_ENCODING.bits
                | MagicFlags::NO_CHECK_JSON.bits;

        }
    }

    impl Default for MagicFlags {
        /// Returns `NONE`
        fn default() -> MagicFlags {
            MagicFlags::NONE
        }
    }
}

/// Returns the version of this crate in the format `MAJOR.MINOR.PATCH`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")

    // concat!(env!("CARGO_PKG_VERSION_MAJOR"), ".", env!("CARGO_PKG_VERSION_MINOR"), ".", env!("CARGO_PKG_VERSION_PATCH"))
    // TODO: There's also an optional _PRE part
}

/// The error type used in this crate
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct MagicError {
    pub desc: String,
}

impl error::Error for MagicError {
    fn description(&self) -> &str {
        "internal libmagic error"
    }
}

impl Display for MagicError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.desc)
    }
}

/// Configuration of which `MagicFlags` and magic databases to use
pub struct Magic {
    cookie: *const ffi::Magic,
}

impl Drop for Magic {
    /// Closes the magic database and deallocates any resources used
    fn drop(&mut self) {
        unsafe { ffi::magic_close(self.cookie) }
    }
}

impl Magic {
    fn last_error(&self) -> Option<MagicError> {
        let e = unsafe { ffi::magic_error(self.cookie) };
        if e.is_null() {
            None
        } else {
            Some(MagicError {
                desc: from_c_str_unsafe!(e),
            })
        }
    }

    fn magic_failure(&self) -> MagicError {
        match self.last_error() {
            Some(e) => e,
            None => MagicError {
                desc: String::from("unknown error"),
            },
        }
    }

    /// Returns a textual description of the contents of the `filename`
    pub fn file(&self, filename: &str) -> Result<String, MagicError> {
        let f = CString::new(filename).map_err(|e| MagicError {
            desc: format!("{:?}", e),
        })?;
        let cf = f.as_ptr();
        let s = unsafe { ffi::magic_file(self.cookie, cf) };
        if s.is_null() {
            Err(self.magic_failure())
        } else {
            Ok(from_c_str_unsafe!(s))
        }
    }

    /// Returns a textual description of the contents of the `buffer`
    pub fn buffer(&self, buf: &[u8]) -> Result<String, MagicError> {
        let buffer_len = buf.len() as size_t;
        let pbuffer = buf.as_ptr();
        let s = unsafe { ffi::magic_buffer(self.cookie, pbuffer, buffer_len) };
        if s.is_null() {
            Err(self.magic_failure())
        } else {
            Ok(from_c_str_unsafe!(s))
        }
    }

    /*
    // Returns a textual explanation of the last error, if any
    //
    // You shouldn't need to call this, since you can use the `MagicError` in
    // the `Result` returned by the other functions.
    fn error(&self) -> Option<String> {
        let s = unsafe { ffi::magic_error(self.cookie) };
        if s.is_null() {
            None
        } else {
            Some(from_c_str_unsafe!(s))
        }
    }
    */

    /// Sets the flags to use
    ///
    /// Overwrites any previously set flags, e.g. those from `load()`.
    // TODO: libmagic itself has to magic_getflags, but we could remember them in Magic?
    pub fn set_flags(&self, flags: flags::MagicFlags) -> bool {
        unsafe { ffi::magic_setflags(self.cookie, flags.bits()) != -1 }
    }

    // TODO: check, compile, list and load mostly do the same, refactor!
    // TODO: ^ also needs to implement multiple databases, possibly waiting for the Path reform

    /// Check the validity of entries in the database `filenames`
    pub fn check(&self, filenames: &[&str]) -> Result<(), MagicError> {
        let cstring;
        let dbs = if filenames.len() == 0 {
            ptr::null()
        } else {
            cstring = CString::new(filenames.join(":")).map_err(|e| MagicError {
                desc: format!("{:?}", e),
            })?;
            cstring.as_ptr()
        };

        let rv;
        unsafe {
            rv = ffi::magic_check(self.cookie, dbs);
        }
        if rv == 0 {
            Ok(())
        } else {
            Err(self.magic_failure())
        }
    }

    /// Compiles the given database `filenames` for faster access
    ///
    /// The compiled files created are named from the `basename` of each file argument with '.mgc' appended to it.
    pub fn compile(&self, filenames: &[&str]) -> Result<(), MagicError> {
        let cstring;
        let dbs = if filenames.len() == 0 {
            ptr::null()
        } else {
            cstring = CString::new(filenames.join(":")).map_err(|e| MagicError {
                desc: format!("{:?}", e),
            })?;
            cstring.as_ptr()
        };

        if unsafe { ffi::magic_compile(self.cookie, dbs) } == 0 {
            Ok(())
        } else {
            Err(self.magic_failure())
        }
    }

    /// Dumps all magic entries in the given database `filenames` in a human readable format
    pub fn list(&self, filenames: &[&str]) -> Result<(), MagicError> {
        let cstring;
        let dbs = if filenames.len() == 0 {
            ptr::null()
        } else {
            cstring = CString::new(filenames.join(":")).map_err(|e| MagicError {
                desc: format!("{:?}", e),
            })?;
            cstring.as_ptr()
        };

        if unsafe { ffi::magic_list(self.cookie, dbs) } == 0 {
            Ok(())
        } else {
            Err(self.magic_failure())
        }
    }

    // Loads the given database `filenames` for further queries.
    //
    // Adds ".mgc" to the database filenames as appropriate.
    fn load(&self, filenames: &[&str]) -> Result<(), MagicError> {
        let cstring;
        let dbs = if filenames.len() == 0 {
            ptr::null()
        } else {
            cstring = CString::new(filenames.join(":")).map_err(|e| MagicError {
                desc: format!("{:?}", e),
            })?;
            cstring.as_ptr()
        };

        if unsafe { ffi::magic_load(self.cookie, dbs) } == 0 {
            Ok(())
        } else {
            Err(self.magic_failure())
        }
    }

    // Loads one or several buffers loaded with contents of compiled magic
    // databases.  This function can be used in environments where the magic
    // library doesn't have direct access to the filesystem.
    fn load_buffers(&self, buffers: &[&[u8]]) -> Result<(), MagicError> {
        let mut ffi_buffers: Vec<*const u8> = Vec::with_capacity(buffers.len());
        let mut ffi_sizes: Vec<libc::size_t> = Vec::with_capacity(buffers.len());
        let ffi_nbuffers = buffers.len() as libc::size_t;

        for slice in buffers {
            ffi_buffers.push((*slice).as_ptr());
            ffi_sizes.push(slice.len() as libc::size_t);
        }

        if unsafe { magic_sys::magic_load_buffers(self.cookie, ffi_buffers.as_ptr(), ffi_sizes.as_ptr(), ffi_nbuffers) } == 0 {
            Ok(())
        } else {
            Err(self.magic_failure())
        }
    }

    // Creates a new configuration.  `flags` specifies how other functions
    // should behave.
    //
    // This doesn't `load()` any databases.
    fn open(flags: flags::MagicFlags) -> Result<Magic, MagicError> {
        let cookie = unsafe { ffi::magic_open((flags | flags::MagicFlags::ERROR).bits()) };
        if cookie.is_null() {
            let e = errno();
            Err(MagicError {
                desc: format!("{} ({})", e, e.0),
            })
        } else {
            Ok(Magic { cookie })
        }
    }

    /// Creates a new configuration and loads one or more magic databases
    /// identified in `filenames`.
    ///
    /// Automatically appends ".mgc" to the file names as appropriate.
    pub fn new(flags: flags::MagicFlags, filenames: &[&str]) -> Result<Magic, MagicError> {
        let cookie = Magic::open(flags)?;
        cookie.load(filenames).map(|_| cookie)
    }

    /// Creates a new configuration and loads one or more buffers.
    ///
    /// `flags` specifies how other functions should behave.  `buffers` should be
    /// pre-loaded with the contents of compiled magic databases.
    ///
    /// This function can be used in environments where the magic library doesn't
    /// have direct access to the filesystem.
    pub fn new_from_buffers(flags: flags::MagicFlags, buffers: &[&[u8]]) -> Result<Magic, MagicError> {
        let cookie = Magic::open(flags)?;
        cookie.load_buffers(buffers).map(|_| cookie)
    }
}

#[cfg(test)]
mod tests {
    extern crate regex;

    use self::regex::Regex;
    use super::flags;
    use super::Magic;

    // Using relative paths to test files should be fine, since cargo doc
    // http://doc.crates.io/build-script.html#inputs-to-the-build-script
    // states that cwd == CARGO_MANIFEST_DIR

    #[test]
    fn file() {
        let magic = Magic::new(flags::MagicFlags::NONE, &vec!["data/tests/db-images-png"]).unwrap();

        let path = "data/tests/rust-logo-128x128-blk.png";

        assert_eq!(
            magic.file(&path).ok().unwrap(),
            "PNG image data, 128 x 128, 8-bit colormap, non-interlaced"
        );

        magic.set_flags(flags::MagicFlags::MIME_TYPE);
        assert_eq!(magic.file(&path).ok().unwrap(), "image/png");

        magic.set_flags(flags::MagicFlags::MIME_TYPE | flags::MagicFlags::MIME_ENCODING);
        assert_eq!(magic.file(&path).ok().unwrap(), "image/png; charset=binary");
    }

    #[test]
    fn buffer() {
        let magic = Magic::new(flags::MagicFlags::NONE, &vec!["data/tests/db-python"]).unwrap();

        let s = b"#!/usr/bin/env python\nprint('Hello, world!')";
        assert_eq!(magic.buffer(s).ok().unwrap(), "Python script, ASCII text executable");

        magic.set_flags(flags::MagicFlags::MIME_TYPE);
        assert_eq!(magic.buffer(s).ok().unwrap(), "text/x-python");
    }

    #[test]
    fn file_error() {
        let magic = Magic::new(flags::MagicFlags::NONE | flags::MagicFlags::ERROR, &[]).unwrap();

        let ret = magic.file("non-existent_file.txt");
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap().desc,
            "cannot stat `non-existent_file.txt' (No such file or directory)"
        );
    }

    #[test]
    fn load_default() {
        assert!(Magic::new(flags::MagicFlags::NONE | flags::MagicFlags::ERROR, &[]).is_ok());
    }

    #[test]
    fn load_one() {
        assert!(Magic::new(
            flags::MagicFlags::NONE | flags::MagicFlags::ERROR,
            &vec!["data/tests/db-images-png"]
        )
        .is_ok());
    }

    #[test]
    fn load_multiple() {
        assert!(Magic::new(
            flags::MagicFlags::NONE | flags::MagicFlags::ERROR,
            &vec!["data/tests/db-images-png", "data/tests/db-python",]
        )
        .is_ok());
    }

    #[test]
    fn version() {
        assert!(Regex::new(r"^\d+\.\d+.\d+$").unwrap().is_match(super::version()))
    }
}
