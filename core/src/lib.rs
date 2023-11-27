#![forbid(future_incompatible)]
//#![deny(bad_style, missing_docs)]
#![doc = include_str!("../README.md")]

pub mod ids;
pub mod l2cap;
pub mod types;

use types::{ActionRc, ListRc, OpType};

/// OTS client result
pub type Result<T> = core::result::Result<T, Error>;

/// OTS client error
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// UTF-8 decoding error
    #[error("Invalid UTF8 string: {0}")]
    BadUtf8(#[from] core::str::Utf8Error),
    /// UUID decoding error
    #[error("Invalid UUID: {0}")]
    BadUuid(#[from] uuid::Error),
    /// Not supported function requested
    #[error("Not supported")]
    NotSupported,
    /// Object not found
    #[error("Not found")]
    NotFound,
    /// No response received
    #[error("No response")]
    NoResponse,
    /// Invalid response received
    #[error("Invalid response")]
    BadResponse,
    /// Invalid UUID size
    #[error("Invalid UUID size: {0}")]
    BadUuidSize(usize),
    /// Object list operation failed
    #[error("Object list error: {0:?}")]
    ListError(#[from] ListRc),
    /// Object action operation failed
    #[error("Object action error: {0:?}")]
    ActionError(#[from] ActionRc),
    /// Invalid action features received
    #[error("Invalid action features: {0:08x?}")]
    BadActionFeatures(u32),
    /// Invalid list features received
    #[error("Invalid list features: {0:08x?}")]
    BadListFeatures(u32),
    /// Invalid properties received
    #[error("Invalid properties: {0:08x?}")]
    BadProperties(u32),
    /// Invalid directory flags received
    #[error("Invalid directory flags: {0:02x?}")]
    BadDirFlags(u8),
    /// Not enough data to parse
    #[error("Not enough data ({actual} < {needed})")]
    NotEnoughData {
        /// Actual size
        actual: usize,
        /// Expected size
        needed: usize,
    },
    /// Too many data to parse
    #[error("Too many data ({actual} > {needed})")]
    TooManyData {
        /// Actual size
        actual: usize,
        /// Expected size
        needed: usize,
    },
    /// Invalid operation code received
    #[error("Invalid opcode for {type_}: {code:02x?}")]
    BadOpCode {
        /// Operation type
        type_: OpType,
        /// Operation code
        code: u8,
    },
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::BadUtf8(err.utf8_error())
    }
}

impl Error {
    /// Check that data length is greater or equals needed
    pub fn check_len(actual: usize, needed: usize) -> Result<()> {
        if actual < needed {
            Err(Error::NotEnoughData { actual, needed })
        } else {
            Ok(())
        }
    }

    /// Check that data length is greater or equals needed
    pub fn check_size<T: Sized>(actual: usize) -> Result<()> {
        Self::check_len(actual, core::mem::size_of::<T>())
    }

    /// Check that data length is exact needed
    pub fn check_len_exact(actual: usize, needed: usize) -> Result<()> {
        Self::check_len(actual, needed)?;
        if actual > needed {
            Err(Error::TooManyData { actual, needed })
        } else {
            Ok(())
        }
    }

    /// Check that data length is exact needed
    pub fn check_size_exact<T: Sized>(actual: usize) -> Result<()> {
        Self::check_len_exact(actual, core::mem::size_of::<T>())
    }
}

/// Object sizes (current and allocated)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Sizes {
    /// Current size of object
    pub current: usize,
    /// Allocated size of object
    pub allocated: usize,
}

impl From<&[u8; 8]> for Sizes {
    fn from(raw: &[u8; 8]) -> Self {
        let sizes = types::Sizes::from(raw);
        Self {
            current: sizes.current as _,
            allocated: sizes.allocated as _,
        }
    }
}

impl TryFrom<&[u8]> for Sizes {
    type Error = Error;

    fn try_from(raw: &[u8]) -> Result<Self> {
        Error::check_size_exact::<types::Sizes>(raw.len())?;
        let sizes = types::Sizes::try_from(raw)?;
        Ok(Self {
            current: sizes.current as _,
            allocated: sizes.allocated as _,
        })
    }
}
