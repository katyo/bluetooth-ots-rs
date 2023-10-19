#![forbid(future_incompatible)]
#![deny(bad_style, missing_docs)]
#![doc = include_str!("../README.md")]

mod ids;
mod l2cap;
mod types;

use bluez_async::{
    BluetoothError, BluetoothEvent, BluetoothSession, CharacteristicEvent, CharacteristicId,
    DeviceId,
};
use bytemuck::{Pod, Zeroable};
use futures::stream::StreamExt;
use uuid::Uuid;

use l2cap::{
    L2capSockAddr as SocketAddr, L2capSocket as Socket, L2capStream as Stream, SocketType,
};
use types::{ActionReq, ActionRes, ListReq, ListRes, OpType, Ule48};

pub use l2cap::{Security, SecurityLevel};
pub use types::{
    ActionFeature, ActionRc, DateTime, DirEntries, ListFeature, ListRc, Metadata, Property,
    SortOrder, WriteMode,
};

/// OTS client result
pub type Result<T> = core::result::Result<T, Error>;

/// OTS client error
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Input/output error
    #[error("Input/Output Error: {0}")]
    IoError(#[from] std::io::Error),
    /// Bluetooth error
    #[error("Bluetooth error: {0}")]
    BtError(#[from] BluetoothError),
    /// UTF-8 decoding error
    #[error("Invalid UTF8 string: {0}")]
    Utf8Error(#[from] core::str::Utf8Error),
    /// UUID decoding error
    #[error("Invalid UUID: {0}")]
    UuidError(#[from] uuid::Error),
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
    /// Timeout reached
    #[error("Timeout reached")]
    Timeout,
    /// Object list operation failed
    #[error("Object list error: {0:?}")]
    ListError(#[from] ListRc),
    /// Object action operation failed
    #[error("Object action error: {0:?}")]
    ActionError(#[from] ActionRc),
    /// Invalid properties received
    #[error("Invalid properties: {0:08x?}")]
    InvalidProps(u32),
    /// Invalid directory flags received
    #[error("Invalid directory flags: {0:02x?}")]
    InvalidDirFlags(u8),
    /// Not enough data to parse
    #[error("Not enough data ({actual} < {needed})")]
    NotEnoughData {
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
        Self::Utf8Error(err.utf8_error())
    }
}

/// Object sizes (current and allocated)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C)]
pub struct Sizes {
    /// Current size of object
    pub current: usize,
    /// Allocated size of object
    pub allocated: usize,
}

/// Object Transfer Service (OTS) client
pub struct OtsClient {
    session: BluetoothSession,
    device_id: DeviceId,
    adapter_addr: SocketAddr,
    device_addr: SocketAddr,
    action_features: ActionFeature,
    list_features: ListFeature,
    oacp_chr: CharacteristicId,
    olcp_chr: Option<CharacteristicId>,
    id_chr: Option<CharacteristicId>,
    name_chr: CharacteristicId,
    type_chr: CharacteristicId,
    size_chr: CharacteristicId,
    prop_chr: CharacteristicId,
    crt_chr: Option<CharacteristicId>,
    mod_chr: Option<CharacteristicId>,
}

impl AsRef<BluetoothSession> for OtsClient {
    fn as_ref(&self) -> &BluetoothSession {
        &self.session
    }
}

impl AsRef<DeviceId> for OtsClient {
    fn as_ref(&self) -> &DeviceId {
        &self.device_id
    }
}

impl AsRef<ActionFeature> for OtsClient {
    fn as_ref(&self) -> &ActionFeature {
        &self.action_features
    }
}

impl AsRef<ListFeature> for OtsClient {
    fn as_ref(&self) -> &ListFeature {
        &self.list_features
    }
}

impl OtsClient {
    /// Create new client instance
    pub async fn new(session: &BluetoothSession, device_id: &DeviceId) -> Result<Self> {
        let ots_srv = session
            .get_service_by_uuid(device_id, ids::service::object_transfer)
            .await?;
        log::debug!("Service: {ots_srv:#?}");

        let ots_chrs = session.get_characteristics(&ots_srv.id).await?;
        log::debug!("Characteristics: {ots_chrs:#?}");

        let ots_feature_chr = session
            .get_characteristic_by_uuid(&ots_srv.id, ids::characteristic::ots_feature)
            .await?;
        log::debug!("Feature Char: {ots_feature_chr:#?}");

        let ots_feature_val = session
            .read_characteristic_value(&ots_feature_chr.id)
            .await?;
        log::trace!("Feature Raw: {ots_feature_val:?}");

        let action_features = *bytemuck::from_bytes(&ots_feature_val[0..4]);
        let list_features = *bytemuck::from_bytes(&ots_feature_val[4..8]);
        log::info!("OTS Feature: {action_features:?} {list_features:?}");

        let oacp_chr = session
            .get_characteristic_by_uuid(
                &ots_srv.id,
                ids::characteristic::object_action_control_point,
            )
            .await?;
        log::debug!("OACP Char: {oacp_chr:#?}");
        let oacp_chr = oacp_chr.id;

        let olcp_chr = session
            .get_characteristic_by_uuid(&ots_srv.id, ids::characteristic::object_list_control_point)
            .await
            .map(Some)
            .or_else(|error| {
                if matches!(error, BluetoothError::UuidNotFound { .. }) {
                    Ok(None)
                } else {
                    Err(error)
                }
            })?;
        log::debug!("OLCP Char: {olcp_chr:#?}");
        let olcp_chr = olcp_chr.map(|chr| chr.id);

        let id_chr = session
            .get_characteristic_by_uuid(&ots_srv.id, ids::characteristic::object_id)
            .await
            .map(Some)
            .or_else(|error| {
                if matches!(error, BluetoothError::UuidNotFound { .. }) {
                    Ok(None)
                } else {
                    Err(error)
                }
            })?;
        log::debug!("Id Char: {id_chr:#?}");
        let id_chr = id_chr.map(|chr| chr.id);

        let name_chr = session
            .get_characteristic_by_uuid(&ots_srv.id, ids::characteristic::object_name)
            .await?;
        log::debug!("Name Char: {name_chr:#?}");
        let name_chr = name_chr.id;

        let type_chr = session
            .get_characteristic_by_uuid(&ots_srv.id, ids::characteristic::object_type)
            .await?;
        log::debug!("Type Char: {type_chr:#?}");
        let type_chr = type_chr.id;

        let size_chr = session
            .get_characteristic_by_uuid(&ots_srv.id, ids::characteristic::object_size)
            .await?;
        log::debug!("Size Char: {size_chr:#?}");
        let size_chr = size_chr.id;

        let prop_chr = session
            .get_characteristic_by_uuid(&ots_srv.id, ids::characteristic::object_properties)
            .await?;
        log::debug!("Prop Char: {prop_chr:#?}");
        let prop_chr = prop_chr.id;

        let crt_chr = session
            .get_characteristic_by_uuid(&ots_srv.id, ids::characteristic::object_first_created)
            .await
            .map(Some)
            .or_else(|error| {
                if matches!(error, BluetoothError::UuidNotFound { .. }) {
                    Ok(None)
                } else {
                    Err(error)
                }
            })?;
        log::debug!("Crt Char: {crt_chr:#?}");
        let crt_chr = crt_chr.map(|chr| chr.id);

        let mod_chr = session
            .get_characteristic_by_uuid(&ots_srv.id, ids::characteristic::object_last_modified)
            .await
            .map(Some)
            .or_else(|error| {
                if matches!(error, BluetoothError::UuidNotFound { .. }) {
                    Ok(None)
                } else {
                    Err(error)
                }
            })?;
        log::debug!("Mod Char: {mod_chr:#?}");
        let mod_chr = mod_chr.map(|chr| chr.id);

        let mut adapter_and_device_info = None;
        for adapter_info in session.get_adapters().await? {
            if let Some(device_info) = session
                .get_devices_on_adapter(&adapter_info.id)
                .await?
                .into_iter()
                .find(|device_info| &device_info.id == device_id)
            {
                adapter_and_device_info = Some((adapter_info, device_info));
            }
        }
        let (adapter_info, device_info) = adapter_and_device_info.ok_or_else(|| Error::NotFound)?;

        let adapter_addr =
            SocketAddr::new_le_dyn_start(adapter_info.mac_address, adapter_info.address_type);

        let device_addr =
            SocketAddr::new_le_cid_ots(device_info.mac_address, device_info.address_type);

        Ok(Self {
            session: session.clone(),
            device_id: device_id.clone(),
            adapter_addr,
            device_addr,
            action_features,
            list_features,
            oacp_chr,
            olcp_chr,
            id_chr,
            name_chr,
            type_chr,
            size_chr,
            prop_chr,
            crt_chr,
            mod_chr,
        })
    }

    /// Get object action feature flags
    pub fn action_features(&self) -> &ActionFeature {
        &self.action_features
    }

    /// Get object list feature flags
    pub fn list_features(&self) -> &ListFeature {
        &self.list_features
    }

    /// Get current object identifier
    pub async fn id(&self) -> Result<Option<u64>> {
        if let Some(chr) = &self.id_chr {
            let raw = self.session.read_characteristic_value(chr).await?;
            Ok(Some(Ule48::try_from(raw.as_ref())?.into()))
        } else {
            Ok(None)
        }
    }

    /// Get current object name
    pub async fn name(&self) -> Result<String> {
        Ok(String::from_utf8(
            self.session
                .read_characteristic_value(&self.name_chr)
                .await?,
        )?)
    }

    /// Get current object type
    pub async fn type_(&self) -> Result<Uuid> {
        let raw = self
            .session
            .read_characteristic_value(&self.type_chr)
            .await?;
        types::uuid_from_raw(raw.as_ref())
    }

    /// Get sizes of current object
    pub async fn size(&self) -> Result<Sizes> {
        let raw_sizes = self
            .session
            .read_characteristic_value(&self.size_chr)
            .await?;
        let sizes: &types::Sizes = bytemuck::from_bytes(&raw_sizes);
        Ok(Sizes {
            current: sizes.current as _,
            allocated: sizes.allocated as _,
        })
    }

    /// Get first created time for selected object
    pub async fn first_created(&self) -> Result<Option<DateTime>> {
        if let Some(chr) = &self.crt_chr {
            let raw = self.session.read_characteristic_value(chr).await?;
            DateTime::try_from(raw.as_slice()).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Get last modified time for selected object
    pub async fn last_modified(&self) -> Result<Option<DateTime>> {
        if let Some(chr) = &self.mod_chr {
            let raw = self.session.read_characteristic_value(chr).await?;
            DateTime::try_from(raw.as_slice()).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Get current object properties
    pub async fn properties(&self) -> Result<Property> {
        let raw = self
            .session
            .read_characteristic_value(&self.prop_chr)
            .await?;
        Property::try_from(raw.as_ref())
    }

    /// Get current object metadata
    pub async fn metadata(&self) -> Result<Metadata> {
        let id = self.id().await?;
        let name = self.name().await?;
        let type_ = self.type_().await?;
        let (current_size, allocated_size) = if let Ok(size) = self.size().await {
            (Some(size.current), Some(size.allocated))
        } else {
            (None, None)
        };
        let first_created = self.first_created().await?;
        let last_modified = self.last_modified().await?;
        let properties = self.properties().await.unwrap_or_default();

        Ok(Metadata {
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

    /// Select previous object
    ///
    /// Returns `false` if current object is first.
    pub async fn previous(&self) -> Result<bool> {
        match self.do_previous().await {
            Ok(_) => Ok(true),
            Err(Error::ListError(ListRc::OutOfBounds)) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Select next object
    ///
    /// Returns `false` if current object is last.
    pub async fn next(&self) -> Result<bool> {
        match self.do_next().await {
            Ok(_) => Ok(true),
            Err(Error::ListError(ListRc::OutOfBounds)) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Select object by identifier
    ///
    /// Returns `false` if object nor found.
    pub async fn go_to(&self, id: u64) -> Result<bool> {
        match self.do_go_to(id).await {
            Ok(_) => Ok(true),
            Err(Error::ListError(ListRc::ObjectIdNotFound)) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn socket(&self) -> Result<Stream> {
        let socket = Socket::new(SocketType::SEQPACKET)?;
        socket.set_security(&l2cap::Security {
            level: l2cap::SecurityLevel::Medium,
            ..Default::default()
        })?;
        log::debug!("{:?}", socket.security()?);
        log::debug!("Bind to {:?}", self.adapter_addr);
        socket.bind(&self.adapter_addr)?;
        log::debug!("Connect to {:?}", self.device_addr);
        let stream = tokio::time::timeout(
            core::time::Duration::from_secs(2),
            socket.connect(&self.device_addr),
        )
        .await
        .map_err(|_| Error::Timeout)??;
        log::debug!(
            "Local/Peer Address: {:?}/{:?}",
            stream.local_addr()?,
            stream.peer_addr()?
        );
        log::debug!(
            "Send/Recv MTU: {:?}/{}",
            stream.send_mtu(),
            stream.recv_mtu()?
        );
        log::debug!("Security: {:?}", stream.security()?);
        Ok(stream)
    }

    /// Read object data
    pub async fn read(&self, offset: usize, length: Option<usize>) -> Result<Vec<u8>> {
        use tokio::io::AsyncReadExt;

        let length = if let Some(length) = length {
            length
        } else {
            self.size().await?.current
        };

        let mut buffer = Vec::with_capacity(length);
        #[allow(clippy::uninit_vec)]
        unsafe {
            buffer.set_len(length)
        };

        let mut stm = self.read_base(offset, length).await?;

        stm.read_exact(&mut buffer[..length]).await?;

        Ok(buffer)
    }

    /// Read object data
    pub async fn read_to(&self, offset: usize, buffer: &mut [u8]) -> Result<usize> {
        use tokio::io::AsyncReadExt;

        let size = self.size().await?.current;

        // length cannot exceeds available length from offset to end
        let length = buffer.len().min(size - offset);

        let mut stm = self.read_base(offset, length).await?;

        stm.read_exact(&mut buffer[..length]).await?;

        Ok(length)
    }

    /// Read object data
    pub async fn read_stream(&self, offset: usize, length: Option<usize>) -> Result<Stream> {
        let size = self.size().await?.current;

        // length cannot exceeds available length from offset to end
        let length = length.unwrap_or(size).min(size - offset);

        self.read_base(offset, length).await
    }

    async fn read_base(&self, offset: usize, length: usize) -> Result<Stream> {
        let stm = self.socket().await?;

        self.do_read(offset, length).await?;

        log::debug!("recv/send mtu: {}/{}", stm.recv_mtu()?, stm.send_mtu()?);

        Ok(stm)
    }

    /// Write object data
    pub async fn write(&self, offset: usize, buffer: &[u8], mode: WriteMode) -> Result<usize> {
        use tokio::io::AsyncWriteExt;

        let size = self.size().await?.allocated;

        // length cannot exceeds available length from offset to end
        let length = buffer.len().min(size - offset);

        let mut stm = self.write_base(offset, length, mode).await?;

        stm.write_all(&buffer[..length]).await?;

        Ok(length)
    }

    /// Write object data
    pub async fn write_stream(
        &self,
        offset: usize,
        length: Option<usize>,
        mode: WriteMode,
    ) -> Result<Stream> {
        let size = self.size().await?.allocated;

        // length cannot exceeds available length from offset to end
        let length = length.unwrap_or(size).min(size - offset);

        self.write_base(offset, length, mode).await
    }

    async fn write_base(&self, offset: usize, length: usize, mode: WriteMode) -> Result<Stream> {
        let stm = self.socket().await?;

        self.do_write(offset, length, mode).await?;

        log::debug!("recv/send mtu: {}/{}", stm.recv_mtu()?, stm.send_mtu()?);

        Ok(stm)
    }

    async fn request(&self, chr: &CharacteristicId, req: impl Into<Vec<u8>>) -> Result<Vec<u8>> {
        self.session.start_notify(chr).await?;

        let resps = self
            .session
            .device_event_stream(&self.device_id)
            .await?
            .filter_map(|event| {
                log::trace!("Evt: {event:?}");
                core::future::ready(
                    if let BluetoothEvent::Characteristic {
                        id,
                        event: CharacteristicEvent::Value { value },
                    } = event
                    {
                        if &id == chr {
                            Some(value)
                        } else {
                            None
                        }
                    } else {
                        None
                    },
                )
            })
            .take(1)
            .take_until(tokio::time::sleep(core::time::Duration::from_secs(1)));
        futures::pin_mut!(resps);

        let req = req.into();
        log::trace!("Req: {req:?}");

        self.session.write_characteristic_value(chr, req).await?;

        let res = resps.next().await.ok_or_else(|| Error::NoResponse)?;
        {
            log::trace!("Res: {res:?}");
        }

        self.session.stop_notify(chr).await?;

        Ok(res)
    }
}

macro_rules! impl_fns {
    ($($req_func:ident: $req_type:ident => $res_type:ident [ $char_field:ident $(: $char_kind:ident)*, $feat_field:ident: $feat_type:ident ] {
        $($(#[$($meta:meta)*])*
          $vis:vis $func:ident: $req_name:ident $({ $($req_arg_name:ident: $req_arg_type:ty),* })* => $res_name:ident $({ $($res_arg_name:ident: $res_arg_type:ty),* })* $([ $feat_name:ident ])*,)*
    })*) => {
        $(
            async fn $req_func(&self, req: &$req_type) -> Result<$res_type> {
                let res = self.request(impl_fns!(# self.$char_field $(: $char_kind)*), req).await?;
                res.as_slice().try_into()
            }

            $(
                $(#[$($meta)*])*
                $vis async fn $func(&self $($(, $req_arg_name: $req_arg_type)*)*) -> Result<impl_fns!(@ $($($res_arg_type)*)*)> {
                    $(if !self.$feat_field.contains($feat_type::$feat_name) {
                        return Err(Error::NotSupported);
                    })*
                    if let $res_type::$res_name $({ $($res_arg_name),* })* = self.$req_func(&$req_type::$req_name $({ $($req_arg_name),* })*).await? {
                        Ok(impl_fns!(@ $($($res_arg_name)*)*))
                    } else {
                        Err(Error::BadResponse)
                    }
                }
            )*
        )*
    };

    (@ $id:ident) => {
        $id
    };

    (@ $type:ty) => {
        $type
    };

    (@ ) => {
        ()
    };

    (# $self:ident . $char_field:ident) => {
        &$self.$char_field
    };

    (# $self:ident . $char_field:ident: Option) => {
        $self.$char_field.as_ref().ok_or_else(|| Error::NotSupported)?
    };
}

impl OtsClient {
    impl_fns! {
        action_request: ActionReq => ActionRes [oacp_chr, action_features: ActionFeature] {
            /// Create new object
            pub create: Create { size: usize, type_: Uuid } => None [Create],
            /// Delete selected object
            pub delete: Delete => None [Delete],
            /// Calculate checksum using selected object data
            pub check_sum: CheckSum { offset: usize, length: usize } => CheckSum { value: u32 } [CheckSum],
            /// Execute selected object
            pub execute: Execute { param: Vec<u8> } => Execute { param: Vec<u8> } [Execute],
            do_read: Read { offset: usize, length: usize } => None [Read],
            do_write: Write { offset: usize, length: usize, mode: WriteMode } => None [Write],
            /// Abort operation
            pub abort: Abort => None [Abort],
        }
        list_request: ListReq => ListRes [olcp_chr: Option, list_features: ListFeature] {
            /// Select first object in a list
            pub first: First => None,
            /// Select last object in a list
            pub last: Last => None,
            do_previous: Previous => None,
            do_next: Next => None,
            do_go_to: GoTo { id: u64 } => None [GoTo],
            /// Change objects order in a list
            pub order: Order { order: SortOrder } => None [Order],
            /// Get number of objects in a list
            pub number_of: NumberOf => NumberOf { count: u32 } [NumberOf],
            /// Clear objects mark
            pub clear_mark: ClearMark => None [ClearMark],
        }
    }
}
