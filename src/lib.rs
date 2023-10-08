mod ids;
mod l2cap;
mod types;

use anyhow::Result;
use bluez_async::{
    BluetoothEvent, BluetoothSession, CharacteristicEvent, CharacteristicId, DeviceId,
};
use bytemuck::{Pod, Zeroable};
use futures::stream::StreamExt;
use uuid::Uuid;

use l2cap::{
    L2capSockAddr as SocketAddr, L2capSocket as Socket, L2capStream as Stream, SocketType,
};
use types::{OacpReq, OacpRes, OlcpReq, OlcpRes, Ule48};

pub use l2cap::{Security, SecurityLevel};
pub use types::{ActionFeature, ListFeature, Metadata, Property, SortOrder, WriteMode};

/// Object sizes (current and allocated)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C)]
pub struct Sizes {
    pub current: usize,
    pub allocated: usize,
}

/// Object Transfer Service (OTS) client
pub struct OtsClient {
    session: BluetoothSession,
    device_id: DeviceId,
    adapter_addr: SocketAddr,
    device_addr: SocketAddr,
    oacp_feat: ActionFeature,
    olcp_feat: ListFeature,
    oacp_chr: CharacteristicId,
    olcp_chr: CharacteristicId,
    id_chr: CharacteristicId,
    name_chr: CharacteristicId,
    type_chr: CharacteristicId,
    size_chr: CharacteristicId,
    prop_chr: CharacteristicId,
}

impl OtsClient {
    /// Create new client instance
    pub async fn new(session: &BluetoothSession, device_id: &DeviceId) -> Result<Self> {
        let ots_srv = session
            .get_service_by_uuid(&device_id, ids::service::object_transfer)
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

        let oacp_feat = *bytemuck::from_bytes(&ots_feature_val[0..4]);
        let olcp_feat = *bytemuck::from_bytes(&ots_feature_val[4..8]);
        log::info!("OTS Feature: {oacp_feat:?} {olcp_feat:?}");

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
            .await?;
        log::debug!("OLCP Char: {olcp_chr:#?}");
        let olcp_chr = olcp_chr.id;

        let id_chr = session
            .get_characteristic_by_uuid(&ots_srv.id, ids::characteristic::object_id)
            .await?;
        log::debug!("Id Char: {id_chr:#?}");
        let id_chr = id_chr.id;

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

        let mut adapter_and_device_info = None;
        for adapter_info in session.get_adapters().await? {
            if let Some(device_info) = session
                .get_devices_on_adapter(&adapter_info.id)
                .await?
                .into_iter()
                .filter(|device_info| &device_info.id == device_id)
                .next()
            {
                adapter_and_device_info = Some((adapter_info, device_info));
            }
        }
        let (adapter_info, device_info) = adapter_and_device_info
            .ok_or_else(|| anyhow::anyhow!("Unable to find device adapter pair"))?;

        let adapter_addr =
            SocketAddr::new_le_dyn_start(adapter_info.mac_address, adapter_info.address_type);

        let device_addr =
            SocketAddr::new_le_cid_ots(device_info.mac_address, device_info.address_type);

        Ok(Self {
            session: session.clone(),
            device_id: device_id.clone(),
            adapter_addr,
            device_addr,
            oacp_feat,
            olcp_feat,
            oacp_chr,
            olcp_chr,
            id_chr,
            name_chr,
            type_chr,
            size_chr,
            prop_chr,
        })
    }

    /// Get object action feature flags
    pub fn action_feature(&self) -> &ActionFeature {
        &self.oacp_feat
    }

    /// Get object list feature flags
    pub fn list_feature(&self) -> &ListFeature {
        &self.olcp_feat
    }

    /// Get current object identifier
    pub async fn id(&self) -> Result<u64> {
        let raw = self.session.read_characteristic_value(&self.id_chr).await?;
        Ok(Ule48::try_from(raw.as_ref())?.into())
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

    /// Get current object properties
    pub async fn props(&self) -> Result<Property> {
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
        let first_created = None; // TODO
        let last_modified = None; // TODO
        let properties = self.props().await.unwrap_or_default();

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
        .map_err(|_| anyhow::anyhow!("Connection timedout"))??;
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
        unsafe { buffer.set_len(length) };

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

    async fn oacp_op(&self, req: &OacpReq) -> Result<OacpRes> {
        let res = self.request(&self.oacp_chr, req).await?;
        res.as_slice().try_into()
    }

    async fn olcp_op(&self, req: &OlcpReq) -> Result<OlcpRes> {
        let res = self.request(&self.olcp_chr, req).await?;
        res.as_slice().try_into()
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

        self.session.write_characteristic_value(&chr, req).await?;

        let res = resps
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("No response"))?;
        {
            log::trace!("Res: {res:?}");
        }

        self.session.stop_notify(chr).await?;

        Ok(res)
    }
}

macro_rules! impl_fns {
    ($($f:ident: $q:ident => $r:ident {
        $($vis:vis $fn:ident: $qn:ident $({ $($qa:ident: $qt:ty),* })* => $rn:ident $({ $($ra:ident: $rt:ty),* })*,)*
    })*) => {
        $(
            $(
                $vis async fn $fn(&self $($(, $qa: $qt)*)*) -> Result<impl_fns!(@ $($($rt)*)*)> {
                    if let $r::$rn $({ $($ra),* })* = self.$f(&$q::$qn $({ $($qa),* })*).await? {
                        Ok(impl_fns!(@ $($($ra)*)*))
                    } else {
                        Err(anyhow::anyhow!("Unexpected response"))
                    }
                }
            )*
        )*
    };

    (@ $i:ident) => {
        $i
    };

    (@ $t:ty) => {
        $t
    };

    (@ ) => {
        ()
    };
}

impl OtsClient {
    impl_fns! {
        oacp_op: OacpReq => OacpRes {
            pub create: Create { size: usize, type_: Uuid } => None,
            pub delete: Delete => None,
            pub check_sum: CheckSum { offset: usize, length: usize } => CheckSum { value: u32 },
            pub execute: Execute { param: Vec<u8> } => Execute { param: Vec<u8> },
            do_read: Read { offset: usize, length: usize } => None,
            do_write: Write { offset: usize, length: usize, mode: WriteMode } => None,
            pub abort: Abort => None,
        }
        olcp_op: OlcpReq => OlcpRes {
            pub first: First => None,
            pub last: Last => None,
            pub previous: Previous => None,
            pub next: Next => None,
            pub go_to: GoTo { id: u64 } => None,
            pub order: Order { order: SortOrder } => None,
            pub number_of: NumberOf => NumberOf { count: u32 },
            pub clear_mark: ClearMark => None,
        }
    }
}
