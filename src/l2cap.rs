use bluez_async::{AddressType, MacAddress};
use core::{
    mem::{size_of, MaybeUninit},
    pin::Pin,
    task::{Context, Poll},
};
use std::{
    io::{Error, Result},
    os::fd::{AsRawFd, RawFd},
};

pub use socket2::Type as SocketType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct L2capSockAddr {
    pub addr: MacAddress,
    pub type_: AddressType,
    pub psm: u16,
    pub cid: u16,
}

#[derive(Clone, Copy)]
#[repr(i32)]
enum BTPROTO {
    L2CAP = 0,
    RFCOMM = 3,
}

const PSM_L2CAP_LE_CID_OTS: u16 = 0x25;
const PSM_L2CAP_LE_DYN_START: u16 = 0x80;

const BT_SNDMTU: i32 = 12;
const BT_RCVMTU: i32 = 13;

impl L2capSockAddr {
    pub fn new(addr: MacAddress, type_: AddressType, psm: u16) -> Self {
        Self {
            addr,
            type_,
            psm,
            cid: 0,
        }
    }

    pub fn new_le_cid_ots(mac_address: MacAddress, address_type: AddressType) -> L2capSockAddr {
        Self::new(mac_address, address_type, PSM_L2CAP_LE_CID_OTS)
    }

    pub fn new_le_dyn_start(mac_address: MacAddress, address_type: AddressType) -> L2capSockAddr {
        Self::new(mac_address, address_type, PSM_L2CAP_LE_DYN_START)
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
        sockaddr_ref.l2_bdaddr_type = match sockaddr.type_ {
            AddressType::Public => 1,
            AddressType::Random => 2,
        };

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
        let addr = <[u8; 6]>::from(*macaddr);

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
    Sdp = 0,
    #[default]
    Low = 1,
    Medium = 2,
    High = 3,
    Fips = 4,
}

pub struct L2capSocket {
    inner: socket2::Socket,
}

impl L2capSocket {
    pub fn new(type_: SocketType) -> Result<Self> {
        let inner = socket2::Socket::new(
            libc::AF_BLUETOOTH.into(),
            type_,
            Some((BTPROTO::L2CAP as i32).into()),
        )?;
        Ok(Self { inner })
    }

    pub fn bind(&self, sockaddr: &L2capSockAddr) -> Result<()> {
        self.inner.bind(&sockaddr.into())
    }

    pub async fn connect(self, sockaddr: &L2capSockAddr) -> Result<L2capStream> {
        self.inner.connect(&sockaddr.into())?;
        self.inner.set_nonblocking(true)?;
        let inner = tokio::io::unix::AsyncFd::new(self)?;

        // Once we've connected, wait for the stream to be writable as
        // that's when the actual connection has been initiated. Once we're
        // writable we check for `take_socket_error` to see if the connect
        // actually hit an error or not.
        //
        // If all that succeeded then we ship everything on up.
        let _ = core::future::poll_fn(|cx| inner.poll_write_ready(cx)).await?;

        if let Some(e) = inner.get_ref().inner.take_error()? {
            return Err(e);
        }

        Ok(L2capStream { inner })
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
        Ok(getsockopt::<u16>(&self.inner, libc::SOL_BLUETOOTH, BT_RCVMTU)? as _)
    }

    pub fn set_recv_mtu(&self, mtu: usize) -> Result<()> {
        let mtu = mtu as u16;
        setsockopt(&self.inner, libc::SOL_BLUETOOTH, BT_RCVMTU, &mtu)
    }

    pub fn send_mtu(&self) -> Result<usize> {
        Ok(getsockopt::<u16>(&self.inner, libc::SOL_BLUETOOTH, BT_SNDMTU)? as _)
    }

    pub fn set_send_mtu(&self, mtu: usize) -> Result<()> {
        let mtu = mtu as u16;
        setsockopt(&self.inner, libc::SOL_BLUETOOTH, BT_SNDMTU, &mtu)
    }
}

impl AsRawFd for L2capSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

pub struct L2capStream {
    inner: tokio::io::unix::AsyncFd<L2capSocket>,
}

impl core::ops::Deref for L2capStream {
    type Target = L2capSocket;
    fn deref(&self) -> &Self::Target {
        self.inner.get_ref()
    }
}

impl tokio::io::AsyncRead for L2capStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        use std::io::Read;

        loop {
            let mut guard = futures::ready!(self.inner.poll_read_ready_mut(cx)?);

            let unfilled = buf.initialize_unfilled();
            match guard.try_io(|inner| {
                inner
                    .get_mut()
                    .inner
                    .read(unsafe { &mut *(unfilled as *mut _ as *mut _) })
            }) {
                Ok(Ok(len)) => {
                    buf.advance(len);
                    return Poll::Ready(Ok(()));
                }
                Ok(Err(err)) => return Poll::Ready(Err(err)),
                Err(_would_block) => continue,
            }
        }
    }
}

impl tokio::io::AsyncWrite for L2capStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        //use std::io::Write;

        loop {
            let mut guard = futures::ready!(self.inner.poll_write_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().inner.send(buf)) {
                Ok(result) => return Poll::Ready(result),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        // tcp flush is a no-op
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.inner
            .get_ref()
            .inner
            .shutdown(std::net::Shutdown::Write)?;
        Poll::Ready(Ok(()))
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
