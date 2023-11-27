use core::mem::{size_of, MaybeUninit};
use std::{
    io::{Error, Result},
    os::fd::{AsRawFd, RawFd},
};

pub use macaddr::MacAddr6 as MacAddress;
pub use socket2::Type as SocketType;

/// Address type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum AddressType {
    Public = 1,
    Random = 2,
}

/// L2CAP socket address
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct L2capSockAddr {
    /// MAC address
    pub addr: MacAddress,
    /// Address type
    pub type_: AddressType,
    /// PSM
    pub psm: u16,
    /// CID
    pub cid: u16,
}

#[derive(Clone, Copy)]
#[repr(i32)]
#[non_exhaustive]
enum BtProto {
    L2Cap = 0,
}

#[derive(Clone, Copy)]
#[repr(u16)]
#[non_exhaustive]
pub enum Psm {
    L2CapLeCidOts = 0x25,
    L2CapLeDynStart = 0x80,
}

impl From<Psm> for u16 {
    fn from(psm: Psm) -> u16 {
        psm as _
    }
}

#[derive(Clone, Copy)]
#[repr(i32)]
#[non_exhaustive]
enum SockOpt {
    BtSndMtu = 12,
    BtRcvMtu = 13,
}

impl L2capSockAddr {
    /// Create L2CAP socket address
    pub fn new(addr: MacAddress, type_: AddressType, psm: impl Into<u16>) -> Self {
        Self {
            addr,
            type_,
            psm: psm.into(),
            cid: 0,
        }
    }
}

#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
#[repr(C)]
struct sockaddr_l2 {
    pub l2_family: libc::sa_family_t,
    pub l2_psm: libc::c_ushort,
    pub l2_bdaddr: bdaddr_t,
    pub l2_cid: libc::c_ushort,
    pub l2_bdaddr_type: u8,
}

impl From<&L2capSockAddr> for socket2::SockAddr {
    fn from(sockaddr: &L2capSockAddr) -> Self {
        let mut addr_storage: libc::sockaddr_storage = unsafe { core::mem::zeroed() };
        let sockaddr_ref = unsafe { &mut *(&mut addr_storage as *mut _ as *mut sockaddr_l2) };

        sockaddr_ref.l2_family = libc::AF_BLUETOOTH as _;
        sockaddr_ref.l2_psm = sockaddr.psm.to_le();
        sockaddr_ref.l2_bdaddr = bdaddr_t::from(&sockaddr.addr);
        sockaddr_ref.l2_cid = sockaddr.cid;
        sockaddr_ref.l2_bdaddr_type = sockaddr.type_ as _;

        unsafe { socket2::SockAddr::new(addr_storage, size_of::<sockaddr_l2>() as _) }
    }
}

impl TryFrom<socket2::SockAddr> for L2capSockAddr {
    type Error = Error;
    fn try_from(sockaddr: socket2::SockAddr) -> Result<Self> {
        let addr_storage = sockaddr.as_storage();
        let sockaddr_ref = unsafe { &*(&addr_storage as *const _ as *const sockaddr_l2) };

        if sockaddr_ref.l2_family != libc::AF_BLUETOOTH as _ {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                "Bluetooth address family expected",
            ));
        }

        Ok(Self {
            addr: sockaddr_ref.l2_bdaddr.into(),
            type_: match sockaddr_ref.l2_bdaddr_type {
                1 => AddressType::Public,
                2 => AddressType::Random,
                _ => {
                    return Err(Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Unknown L2CAP address type",
                    ))
                }
            },
            psm: sockaddr_ref.l2_psm,
            cid: sockaddr_ref.l2_cid,
        })
    }
}

#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
#[repr(C, packed)]
struct bdaddr_t {
    pub b: [u8; 6],
}

impl From<&MacAddress> for bdaddr_t {
    fn from(macaddr: &MacAddress) -> Self {
        let addr = macaddr.into_array();

        #[cfg(target_endian = "little")]
        let addr = {
            let mut addr = addr;
            addr.reverse();
            addr
        };

        Self { b: addr }
    }
}

impl From<&bdaddr_t> for MacAddress {
    fn from(bdaddr: &bdaddr_t) -> Self {
        let addr = bdaddr.b;

        #[cfg(target_endian = "little")]
        let addr = {
            let mut addr = addr;
            addr.reverse();
            addr
        };

        Self::from(addr)
    }
}

impl From<bdaddr_t> for MacAddress {
    fn from(bdaddr: bdaddr_t) -> Self {
        Self::from(&bdaddr)
    }
}

/// Bluetooth security
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(C)]
pub struct Security {
    /// Level.
    pub level: SecurityLevel,
    /// Key size
    pub key_size: u8,
}

pub const BT_SECURITY: i32 = 4;

/// Bluetooth security level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum SecurityLevel {
    /// SPD
    Sdp = 0,
    /// Low (default)
    #[default]
    Low = 1,
    /// Medium
    Medium = 2,
    /// High
    High = 3,
    /// FIPS
    Fips = 4,
}

pub struct L2capSocket {
    inner: socket2::Socket,
}

impl core::ops::Deref for L2capSocket {
    type Target = socket2::Socket;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl core::ops::DerefMut for L2capSocket {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl L2capSocket {
    pub fn new(type_: SocketType) -> Result<Self> {
        let inner = socket2::Socket::new(
            libc::AF_BLUETOOTH.into(),
            type_,
            Some((BtProto::L2Cap as i32).into()),
        )?;
        Ok(Self { inner })
    }

    pub fn bind(&self, sockaddr: &L2capSockAddr) -> Result<()> {
        self.inner.bind(&sockaddr.into())
    }

    pub fn connect(&self, sockaddr: &L2capSockAddr) -> Result<()> {
        self.inner.connect(&sockaddr.into())
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }

    /// Get the local address of this socket.
    pub fn local_addr(&self) -> Result<L2capSockAddr> {
        self.inner.local_addr().and_then(TryFrom::try_from)
    }

    /// Get the peer address of this socket.
    pub fn peer_addr(&self) -> Result<L2capSockAddr> {
        self.inner.peer_addr().and_then(TryFrom::try_from)
    }

    pub fn security(&self) -> Result<Security> {
        getsockopt(&self.inner, libc::SOL_BLUETOOTH, BT_SECURITY)
    }

    pub fn set_security(&self, security: &Security) -> Result<()> {
        setsockopt(&self.inner, libc::SOL_BLUETOOTH, BT_SECURITY, security)
    }

    pub fn recv_mtu(&self) -> Result<usize> {
        Ok(getsockopt::<u16>(&self.inner, libc::SOL_BLUETOOTH, SockOpt::BtRcvMtu as _)? as _)
    }

    pub fn set_recv_mtu(&self, mtu: usize) -> Result<()> {
        let mtu = mtu as u16;
        setsockopt(
            &self.inner,
            libc::SOL_BLUETOOTH,
            SockOpt::BtRcvMtu as _,
            &mtu,
        )
    }

    pub fn send_mtu(&self) -> Result<usize> {
        Ok(getsockopt::<u16>(&self.inner, libc::SOL_BLUETOOTH, SockOpt::BtSndMtu as _)? as _)
    }

    pub fn set_send_mtu(&self, mtu: usize) -> Result<()> {
        let mtu = mtu as u16;
        setsockopt(
            &self.inner,
            libc::SOL_BLUETOOTH,
            SockOpt::BtSndMtu as _,
            &mtu,
        )
    }
}

impl AsRawFd for L2capSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

fn getsockopt<T>(socket: &impl AsRawFd, level: libc::c_int, optname: libc::c_int) -> Result<T> {
    let mut optval: MaybeUninit<T> = MaybeUninit::uninit();
    let mut optlen: libc::socklen_t = size_of::<T>() as _;
    if unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            level,
            optname,
            optval.as_mut_ptr() as *mut _,
            &mut optlen,
        )
    } == -1
    {
        return Err(Error::last_os_error());
    }
    if optlen != size_of::<T>() as _ {
        return Err(Error::new(std::io::ErrorKind::InvalidInput, "invalid size"));
    }
    let optval = unsafe { optval.assume_init() };
    Ok(optval)
}

fn setsockopt<T>(
    socket: &impl AsRawFd,
    level: libc::c_int,
    optname: libc::c_int,
    optval: &T,
) -> Result<()> {
    let optlen: libc::socklen_t = size_of::<T>() as _;
    if unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            level,
            optname,
            optval as *const _ as *const _,
            optlen,
        )
    } == -1
    {
        return Err(Error::last_os_error());
    }
    Ok(())
}
