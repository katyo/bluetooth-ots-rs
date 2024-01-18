use core::{
    pin::Pin,
    task::{Context, Poll},
};
use futures_util::ready;
use ots_core::l2cap::{self, L2capSockAddr};
use std::{
    io::Result,
    os::fd::{AsRawFd, RawFd},
};

pub struct L2capSocket {
    inner: l2cap::L2capSocket,
}

impl core::ops::Deref for L2capSocket {
    type Target = l2cap::L2capSocket;

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
    pub fn new(type_: l2cap::SocketType) -> Result<Self> {
        l2cap::L2capSocket::new(type_).map(|inner| Self { inner })
    }

    pub async fn connect(self, sockaddr: &L2capSockAddr) -> Result<L2capStream> {
        self.inner.connect(sockaddr)?;
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
            let mut guard = ready!(self.inner.poll_read_ready_mut(cx)?);

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
        loop {
            let mut guard = ready!(self.inner.poll_write_ready(cx))?;

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
