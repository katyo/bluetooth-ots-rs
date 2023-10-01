use bitflags::bitflags;
use bytemuck::{Pod, Zeroable};
use uuid::Uuid;

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

    /// Object write action mode flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable, Default)]
    #[repr(C)]
    pub struct WriteMode: u8 {
        const Truncate = 1 << 1;
    }
}

/// 48-bit unsigned int type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct U48 {
    raw: [u8; 6],
}

impl From<u64> for U48 {
    fn from(val: u64) -> Self {
        Self {
            raw: *bytemuck::from_bytes(&val.to_ne_bytes()[..6]),
        }
    }
}

impl From<U48> for u64 {
    fn from(val: U48) -> Self {
        let mut res = 0u64;
        unsafe { core::mem::transmute::<_, &mut [u8; 6]>(&mut res) }.copy_from_slice(&val.raw);
        res
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
                type Error = anyhow::Error;

                fn try_from($raw: u8) -> Result<Self, Self::Error> {
                    use $tn::*;
                    if $cond {
                        Ok(unsafe { *(&$raw as *const _ as *const _) })
                    } else {
                        Err(anyhow::anyhow!(concat!("Invalid ", stringify!($tn), " code: {raw:02x?}")))
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
    type Error = anyhow::Error;

    fn try_from(raw: &[u8]) -> Result<Self, Self::Error> {
        use OlcpRes::*;

        if !matches!(raw[0].try_into()?, OlcpOp::Response) {
            return Err(anyhow::anyhow!("Isn't a response"));
        }

        match raw[2].try_into()? {
            OlcpRc::Success => Ok(if matches!(raw[1].try_into()?, OlcpOp::NumberOf) {
                NumberOf {
                    count: *bytemuck::from_bytes(&raw[2..][..4]),
                }
            } else {
                None
            }),
            rc => Err(anyhow::anyhow!("Operation error: {rc}")),
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
    type Error = anyhow::Error;

    fn try_from(raw: &[u8]) -> Result<Self, Self::Error> {
        use OacpRes::*;

        if !matches!(raw[0].try_into()?, OacpOp::Response) {
            return Err(anyhow::anyhow!("Isn't a response"));
        }

        match raw[2].try_into()? {
            OacpRc::Success => Ok(match raw[1].try_into()? {
                OacpOp::CheckSum => CheckSum {
                    value: *bytemuck::from_bytes(&raw[2..][..4]),
                },
                OacpOp::Execute => Execute {
                    param: raw[2..].into(),
                },
                _ => None,
            }),
            rc => Err(anyhow::anyhow!("Operation error: {rc}")),
        }
    }
}
