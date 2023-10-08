use bitflags::bitflags;
use bytemuck::{Pod, Zeroable};
use uuid::Uuid;
use crate::{Error, Result};

bitflags! {
    /// Object action feature flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
    #[repr(C)]
    pub struct ActionFeature: u32 {
        const Create = 1 << 0;
        const Delete = 1 << 1;
        const CheckSum = 1 << 2;
        const Execute = 1 << 3;
        const Read = 1 << 4;
        const Write = 1 << 5;
        const Append = 1 << 6;
        const Truncate = 1 << 7;
        const Patch = 1 << 8;
        const Abort = 1 << 9;
    }

    /// Object list feature flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
    #[repr(C)]
    pub struct ListFeature: u32 {
        const GoTo = 1 << 0;
        const Order = 1 << 1;
        const NumberOf = 1 << 2;
        const ClearMark = 1 << 3;
    }

    /// Object property flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable, Default)]
    #[repr(C)]
    pub struct Property: u32 {
        const Delete = 1 << 0;
        const Execute = 1 << 1;
        const Read = 1 << 2;
        const Write = 1 << 3;
        const Append = 1 << 4;
        const Truncate = 1 << 5;
        const Patch = 1 << 6;
        const Mark = 1 << 7;
    }

    /// Object write action mode flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable, Default)]
    #[repr(C)]
    pub struct WriteMode: u8 {
        const Truncate = 1 << 1;
    }

    /// Object directory flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable, Default)]
    #[repr(C)]
    pub struct DirFlag: u8 {
        const TypeUuid128 = 1 << 0;
        const HasCurrentSize = 1 << 1;
        const HasAllocatedSize = 1 << 2;
        const HasFirstCreated = 1 << 3;
        const HasLastModified = 1 << 4;
        const HasProperties = 1 << 5;
        const HasExtendedFlags = 1 << 7;
    }
}

pub fn uuid_from_raw(raw: &[u8]) -> Result<Uuid> {
    Ok(match raw.len() {
        2 => bluez_async::uuid_from_u16(u16::from_le_bytes(*raw.split_array_ref_().0)),
        4 => bluez_async::uuid_from_u32(u32::from_le_bytes(*raw.split_array_ref_().0)),
        16 => Uuid::from_slice(&raw)?,
        len => return Err(Error::BadUuidSize(len)),
    })
}

impl TryFrom<[u8; 4]> for Property {
    type Error = Error;
    fn try_from(raw: [u8; 4]) -> Result<Self> {
        let val = u32::from_le_bytes(raw);
        Self::from_bits(val)
            .ok_or_else(|| Error::InvalidProps(val))
    }
}

impl TryFrom<&[u8]> for Property {
    type Error = Error;
    fn try_from(raw: &[u8]) -> Result<Self> {
        if raw.len() < 4 {
            return Err(Error::NotEnoughData { actual: raw.len(), needed: 4 });
        }
        let (raw, _) = raw.split_array_ref_();
        Self::try_from(*raw)
    }
}

/// 48-bit unsigned int type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct Ule48 {
    raw: [u8; 6],
}

impl TryFrom<&[u8]> for Ule48 {
    type Error = Error;
    fn try_from(raw: &[u8]) -> Result<Self> {
        if raw.len() < 6 {
            return Err(Error::NotEnoughData { actual: raw.len(), needed: 6 });
        }
        let (raw, _) = raw.split_array_ref_();
        Ok(Self::from(*raw))
    }
}

impl From<[u8; 6]> for Ule48 {
    fn from(raw: [u8; 6]) -> Self {
        Self { raw }
    }
}

impl From<Ule48> for [u8; 6] {
    fn from(Ule48 { raw }: Ule48) -> Self {
        raw
    }
}

impl From<u64> for Ule48 {
    fn from(val: u64) -> Self {
        Self {
            raw: *val.to_le_bytes().as_ref().split_array_ref_().0,
        }
    }
}

impl From<Ule48> for u64 {
    fn from(Ule48 { raw }: Ule48) -> Self {
        Self::from_le_bytes([raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], 0, 0])
    }
}

/// Object sizes (current and allocated)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C)]
pub struct Sizes {
    pub current: u32,
    pub allocated: u32,
}

macro_rules! impl_bc {
    ($( $(#[$($tm:meta)*])* $tn:ident (|$raw:ident| $cond:expr) { $($vn:ident = $vc:literal,)* } )*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u8)]
        pub enum OpType {
            $($tn,)*
        }

        impl AsRef<str> for OpType {
            fn as_ref(&self) -> &str {
                use OpType::*;
                match self {
                    $($tn => stringify!($tn),)*
                }
            }
        }

        impl core::fmt::Display for OpType {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                f.write_str(self.as_ref())
            }
        }

        $(
            $(#[$($tm)*])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            #[repr(u8)]
            pub enum $tn {
                $(#[doc = stringify!($vn)] $vn = $vc,)*
            }

            impl AsRef<str> for $tn {
                fn as_ref(&self) -> &str {
                    use $tn::*;
                    match self {
                        $($vn => stringify!($vn),)*
                    }
                }
            }

            impl core::fmt::Display for $tn {
                fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                    f.write_str(self.as_ref())
                }
            }

            impl TryFrom<u8> for $tn {
                type Error = Error;

                fn try_from($raw: u8) -> Result<Self> {
                    use $tn::*;
                    if $cond {
                        Ok(unsafe { *(&$raw as *const _ as *const _) })
                    } else {
                        Err(Error::BadOpCode { type_: OpType::$tn, code: $raw })
                    }
                }
            }
        )*
    };
}

impl_bc! {
    /// Object List Sort Order
    SortOrder (|raw| raw >= NameAsc as _ && raw <= ModTimeAsc as _ ||
               raw >= NameDesc as _ && raw <= ModTimeDesc as _) {
        NameAsc = 0x01,
        TypeAsc = 0x02,
        CurSizeAsc = 0x03,
        CrtTimeAsc = 0x04,
        ModTimeAsc = 0x05,
        NameDesc = 0x11,
        TypeDesc = 0x12,
        CurSizeDesc = 0x13,
        CrtTimeDesc = 0x14,
        ModTimeDesc = 0x15,
    }

    /// Object list operation code
    OlcpOp (|raw| raw >= First as _ && raw <= ClearMark as _ || raw == Response as _) {
        First = 0x01,
        Last = 0x02,
        Previous = 0x03,
        Next = 0x04,
        GoTo = 0x05,
        Order = 0x06,
        NumberOf = 0x07,
        ClearMark = 0x08,
        Response = 0x70,
    }

    /// Object list operation result code
    OlcpRc (|raw| raw >= Success as _ && raw <= ObjectIdNotFound as _) {
        Success = 0x01,
        OperationNotSupported = 0x02,
        InvalidParameter = 0x03,
        OperationFailed = 0x04,
        OutOfBounds = 0x05,
        TooManyObjects = 0x06,
        NoObject = 0x07,
        ObjectIdNotFound = 0x08,
    }

    /// Object action operation code
    OacpOp (|raw| raw >= Create as _ && raw <= Abort as _ || raw == Response as _) {
        Create = 0x01,
        Delete = 0x02,
        CheckSum = 0x03,
        Execute = 0x04,
        Read = 0x05,
        Write = 0x06,
        Abort = 0x07,
        Response = 0x60,
    }

    /// Object action operation result code
    OacpRc (|raw| raw >= Success as _ && raw <= OperationFailed as _) {
        Success = 0x01,
        OperationNotSupported = 0x02,
        InvalidParameter = 0x03,
        InsufficientResources = 0x04,
        InvalidObject = 0x05,
        ChannelUnavailable = 0x06,
        UnsupportedType = 0x07,
        ProcedureNotPermitted = 0x08,
        ObjectLocked = 0x09,
        OperationFailed = 0x0a,
    }
}

impl std::error::Error for OlcpRc {}
impl std::error::Error for OacpRc {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OlcpReq {
    First,
    Last,
    Previous,
    Next,
    GoTo { id: u64 },
    Order { order: SortOrder },
    NumberOf,
    ClearMark,
}

impl From<&OlcpReq> for Vec<u8> {
    fn from(op: &OlcpReq) -> Self {
        use OlcpReq::*;

        let mut out = Vec::with_capacity(7);
        match op {
            First => out.push(OlcpOp::First as _),
            Last => out.push(OlcpOp::Last as _),
            Previous => out.push(OlcpOp::Previous as _),
            Next => out.push(OlcpOp::Next as _),
            GoTo { id } => {
                out.push(OlcpOp::GoTo as _);
                out.extend_from_slice(&id.to_le_bytes()[..6]);
            }
            Order { order } => {
                out.push(OlcpOp::Order as _);
                out.push(*order as _);
            }
            NumberOf => out.push(OlcpOp::NumberOf as _),
            ClearMark => out.push(OlcpOp::ClearMark as _),
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OlcpRes {
    None,
    NumberOf { count: u32 },
}

impl TryFrom<&[u8]> for OlcpRes {
    type Error = Error;

    fn try_from(raw: &[u8]) -> Result<Self> {
        use OlcpRes::*;

        if raw.len() < 3 {
            return Err(Error::NotEnoughData { actual: raw.len(), needed: 3 });
        }

        if !matches!(raw[0].try_into()?, OlcpOp::Response) {
            return Err(Error::BadResponse);
        }

        match raw[2].try_into()? {
            OlcpRc::Success => Ok(if matches!(raw[1].try_into()?, OlcpOp::NumberOf) {
                if raw.len() < 7 {
                    return Err(Error::NotEnoughData { actual: raw.len(), needed: 77 });
                }
                NumberOf {
                    count: u32::from_le_bytes(*raw[3..].as_ref().split_array_ref_().0),
                }
            } else {
                None
            }),
            rc => Err(Error::ListError(rc)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OacpReq {
    Create {
        size: usize,
        type_: Uuid,
    },
    Delete,
    CheckSum {
        offset: usize,
        length: usize,
    },
    Execute {
        param: Vec<u8>,
    },
    Read {
        offset: usize,
        length: usize,
    },
    Write {
        offset: usize,
        length: usize,
        mode: WriteMode,
    },
    Abort,
}

impl From<&OacpReq> for Vec<u8> {
    fn from(op: &OacpReq) -> Self {
        use OacpReq::*;

        let mut out = Vec::<u8>::with_capacity(10);
        match op {
            Create { size, type_ } => {
                out.push(OacpOp::Create as _);
                out.extend_from_slice(&size.to_le_bytes()[..4]);
                out.extend_from_slice(type_.as_ref());
            }
            Delete => out.push(OacpOp::Delete as _),
            CheckSum { offset, length } => {
                out.push(OacpOp::CheckSum as _);
                out.extend_from_slice(&offset.to_le_bytes()[..4]);
                out.extend_from_slice(&length.to_le_bytes()[..4]);
            }
            Execute { param } => {
                out.push(OacpOp::Execute as _);
                out.extend_from_slice(&param);
            }
            Read { offset, length } => {
                out.push(OacpOp::Read as _);
                out.extend_from_slice(&offset.to_le_bytes()[..4]);
                out.extend_from_slice(&length.to_le_bytes()[..4]);
            }
            Write {
                offset,
                length,
                mode,
            } => {
                out.push(OacpOp::Write as _);
                out.extend_from_slice(&offset.to_le_bytes()[..4]);
                out.extend_from_slice(&length.to_le_bytes()[..4]);
                out.push(mode.bits());
            }
            Abort => out.push(OacpOp::Abort as _),
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OacpRes {
    None,
    CheckSum { value: u32 },
    Execute { param: Vec<u8> },
}

impl TryFrom<&[u8]> for OacpRes {
    type Error = Error;

    fn try_from(raw: &[u8]) -> Result<Self> {
        use OacpRes::*;

        if raw.len() < 3 {
            return Err(Error::NotEnoughData { actual: raw.len(), needed: 3 })
        }

        if !matches!(raw[0].try_into()?, OacpOp::Response) {
            return Err(Error::BadResponse);
        }

        match raw[2].try_into()? {
            OacpRc::Success => Ok(match raw[1].try_into()? {
                OacpOp::CheckSum => {
                    if raw.len() < 7 {
                        return Err(Error::NotEnoughData { actual: raw.len(), needed: 7 });
                    }
                    CheckSum {
                        value: u32::from_le_bytes(*raw[3..].as_ref().split_array_ref_().0),
                    }
                }
                OacpOp::Execute => Execute {
                    param: raw[3..].into(),
                },
                _ => None,
            }),
            rc => Err(Error::ActionError(rc)),
        }
    }
}

/// Object date and time
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl From<&[u8; 7]> for DateTime {
    fn from(raw: &[u8; 7]) -> Self {
        Self {
            year: u16::from_le_bytes([raw[0], raw[1]]),
            month: raw[2],
            day: raw[3],
            hour: raw[4],
            minute: raw[5],
            second: raw[6],
        }
    }
}

impl TryFrom<&[u8]> for DateTime {
    type Error = Error;
    fn try_from(raw: &[u8]) -> Result<Self> {
        if raw.len() < 7 {
            return Err(Error::NotEnoughData { actual: raw.len(), needed: 7 });
        }
        let (dt, _) = raw.split_array_ref_();
        Ok(Self::from(dt))
    }
}

/// Object metadata
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Metadata {
    pub id: u64,
    pub name: String,
    pub type_: Uuid,
    pub current_size: Option<usize>,
    pub allocated_size: Option<usize>,
    pub first_created: Option<DateTime>,
    pub last_modified: Option<DateTime>,
    pub properties: Property,
}

impl Metadata {
    pub fn split_dir_entry(raw: &[u8]) -> Result<Option<(&[u8], &[u8])>> {
        if raw.len() == 0 {
            return Ok(None);
        }
        if raw.len() < 13 {
            return Err(Error::NotEnoughData { actual: raw.len(), needed: 13 });
        }
        let (record_len, _) = raw.split_array_ref_();
        let record_len = u16::from_le_bytes(*record_len) as usize;
        if raw.len() < record_len {
            return Err(Error::NotEnoughData { actual: raw.len(), needed: record_len });
        }
        let (rec, rest) = raw.split_at(record_len);
        Ok(Some((&rec[2..], rest)))
    }
}

impl TryFrom<&[u8]> for Metadata {
    type Error = Error;
    fn try_from(raw: &[u8]) -> Result<Self> {
        if raw.len() < 11 {
            return Err(Error::NotEnoughData { actual: raw.len(), needed: 11 });
        }
        let (id, raw) = raw.split_array_ref_();
        let id: u64 = Ule48::from(*id).into();
        let (name_len, raw) = raw.split_array_ref_();
        let name_len = u8::from_le_bytes(*name_len) as usize;
        if raw.len() < name_len + 1 + 2 {
            return Err(Error::NotEnoughData { actual: raw.len(), needed: name_len + 1 + 2 });
        }
        let (name, raw) = raw.split_at(name_len);
        let name = core::str::from_utf8(name)?.into();
        let (flags, raw) = raw.split_array_ref_();
        let flags = u8::from_le_bytes(*flags);
        let flags = DirFlag::from_bits(flags)
            .ok_or_else(|| Error::InvalidDirFlags(flags))?;
        let (type_, raw) = if flags.contains(DirFlag::TypeUuid128) {
            if raw.len() < 16 {
                return Err(Error::NotEnoughData { actual: raw.len(), needed: 16 });
            }
            let (uuid, raw) = raw.split_array_ref_();
            (Uuid::from_bytes(*uuid), raw)
        } else {
            let (uuid, raw) = raw.split_array_ref_();
            (bluez_async::uuid_from_u16(u16::from_le_bytes(*uuid)), raw)
        };
        let (current_size, raw) = if flags.contains(DirFlag::HasCurrentSize) {
            if raw.len() < 4 {
                return Err(Error::NotEnoughData { actual: raw.len(), needed: 4 });
            }
            let (size, raw) = raw.split_array_ref_();
            let size = u32::from_le_bytes(*size);
            (Some(size as usize), raw)
        } else {
            (None, raw)
        };
        let (allocated_size, raw) = if flags.contains(DirFlag::HasAllocatedSize) {
            if raw.len() < 4 {
                return Err(Error::NotEnoughData { actual: raw.len(), needed: 4 });
            }
            let (size, raw) = raw.split_array_ref_();
            let size = u32::from_le_bytes(*size);
            (Some(size as usize), raw)
        } else {
            (None, raw)
        };
        let (first_created, raw) = if flags.contains(DirFlag::HasFirstCreated) {
            if raw.len() < 7 {
                return Err(Error::NotEnoughData { actual: raw.len(), needed: 7 });
            }
            let (time, raw) = raw.split_array_ref_();
            let time = DateTime::from(time);
            (Some(time), raw)
        } else {
            (None, raw)
        };
        let (last_modified, raw) = if flags.contains(DirFlag::HasFirstCreated) {
            if raw.len() < 7 {
                return Err(Error::NotEnoughData { actual: raw.len(), needed: 7 });
            }
            let (time, raw) = raw.split_array_ref_();
            let time = DateTime::from(time);
            (Some(time), raw)
        } else {
            (None, raw)
        };
        let (properties, _raw) = if flags.contains(DirFlag::HasProperties) {
            if raw.len() < 4 {
                return Err(Error::NotEnoughData { actual: raw.len(), needed: 4 });
            }
            let (props, raw) = raw.split_array_ref_();
            let props = Property::try_from(*props)?;
            (props, raw)
        } else {
            (Property::default(), raw)
        };
        Ok(Self {
            id,
            name,
            type_,
            current_size,
            allocated_size,
            first_created,
            last_modified,
            properties,
        })
    }
}

trait SliceExt {
    type V;
    fn split_array_ref_<const N: usize>(&self) -> (&[Self::V; N], &[Self::V]);
}

impl<T> SliceExt for &[T] {
    type V = T;
    fn split_array_ref_<const N: usize>(&self) -> (&[T; N], &[T]) {
        let (a, b) = self.split_at(N);
        // SAFETY: a points to [T; N]? Yes it's [T] of length N (checked by split_at)
        unsafe { (&*(a.as_ptr() as *const [T; N]), b) }
    }
}
