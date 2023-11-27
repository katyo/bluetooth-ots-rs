use bluez_async::BluetoothSession;
use bluez_async_ots::{ClientConfig, CoreError, DirEntries, OtsClient};
use core::time::Duration;
use either::Either;
use tokio::{io::AsyncReadExt, time::sleep};

mod cli;

/// OTS command result
pub type Result<T> = core::result::Result<T, Error>;

/// OTS command error
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Input/Output Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Bluetooth Error: {0}")]
    BtError(#[from] bluez_async::BluetoothError),
    #[error("OTS Error: {0}")]
    OtsError(#[from] bluez_async_ots::Error),
    #[error("Invalid UTF8 string: {0}")]
    Utf8Error(#[from] core::str::Utf8Error),
    #[error("No adapter found")]
    NoAdapter,
    #[error("No device found")]
    NoDevice,
    #[error("No object found")]
    NoObject,
    #[error("Need any of index, id or name to select object to read")]
    ObjIdError,
    #[error("Bad hexadecimal data")]
    HexError,
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::Utf8Error(err.utf8_error())
    }
}

impl From<CoreError> for Error {
    fn from(err: CoreError) -> Self {
        Error::OtsError(err.into())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let args = <cli::Args as clap::Parser>::parse();

    let config = ClientConfig {
        privileged: args.privileged,
        ..Default::default()
    };

    let (_, bs) = BluetoothSession::new().await?;

    let adapter_id = if let Some(mac_or_name) = &args.adapter {
        Some(
            bs.get_adapters()
                .await?
                .into_iter()
                .find(
                    |adp| match (mac_or_name, &adp.mac_address, &adp.name, &adp.alias) {
                        (Either::Left(req_addr), adp_addr, _, _) if req_addr == adp_addr => true,
                        (Either::Right(req_name), _, adp_name, adp_alias)
                            if req_name == adp_name || req_name == adp_alias =>
                        {
                            true
                        }
                        _ => false,
                    },
                )
                .ok_or_else(|| Error::NoAdapter)?
                .id,
        )
    } else {
        None
    };

    if let Some(secs) = &args.disco {
        if let Some(id) = &adapter_id {
            log::info!("Start discovery on {id:?}");
            bs.start_discovery_on_adapter(id).await?;
        } else {
            log::info!("Start discovery");
            bs.start_discovery().await?;
        }
        sleep(Duration::from_secs(*secs as _)).await;
        if let Some(id) = &adapter_id {
            log::info!("Stop discovery on {id:?}");
            bs.stop_discovery_on_adapter(id).await?;
        } else {
            log::info!("Start discovery");
            bs.stop_discovery().await?;
        }
    }

    let devs = bs.get_devices().await?;

    let dev = devs
        .into_iter()
        .find(|dev| match (&args.device, &dev.mac_address, &dev.name) {
            (Either::Left(req_addr), dev_addr, _) if req_addr == dev_addr => true,
            (Either::Right(req_name), _, Some(dev_name)) if req_name == dev_name => true,
            _ => false,
        })
        .ok_or_else(|| Error::NoDevice)?;
    log::debug!("Device: {dev:#?}");

    let dev_id = dev.id.clone();
    log::info!("Device: {dev_id:?}");

    let connected = dev.connected;

    if !connected {
        log::info!("Connect to device");
        bs.connect_with_timeout(&dev_id, Duration::from_secs(2))
            .await?;
    }

    let ots = OtsClient::new(&bs, &dev_id, &config).await?;

    use cli::Action::*;
    match args.action {
        Info(args) => args.run(&ots).await?,
        List(args) => args.run(&ots).await?,
        Read(args) => args.run(&ots).await?,
        Write(args) => args.run(&ots).await?,
    }

    if !connected {
        log::info!("Disconnect from device");
        bs.disconnect(&dev.id).await?;
    }

    Ok(())
}

impl cli::InfoArgs {
    pub async fn run(&self, ots: &OtsClient) -> Result<()> {
        if self.action() {
            println!("Object action features: {}", ots.action_features());
        }
        if self.list() {
            println!("Object list features: {}", ots.list_features());
        }
        Ok(())
    }
}

impl cli::ListArgs {
    pub async fn run(&self, ots: &OtsClient) -> Result<()> {
        self.print_header();

        // try read special directory object first
        if self.dir && ots.go_to(0).await.is_ok() {
            match ots.read(0, None).await {
                Ok(data) => {
                    log::debug!("Directory data size: {}", data.len());
                    return self.print_directory_data(&data, ots).await;
                }
                Err(error) => {
                    log::warn!("Unable to read directory data due to: {error:?}");
                }
            }
        }

        self.print_directory_iter(ots).await
    }

    fn print_header(&self) {
        print!("index");
        if self.id() {
            print!("\tid");
        }
        if self.name() {
            print!("\tname");
        }
        if self.type_() {
            print!("\ttype");
        }
        if self.any_size() {
            print!("\tsize ");
            if self.cur_size() {
                print!("current");
            }
            if self.alloc_size() {
                if self.cur_size() {
                    print!("/");
                }
                print!("allocated");
            }
        }
        if self.any_time() {
            println!("\ttime ");
            if self.crt_time() {
                print!("created");
            }
            if self.mod_time() {
                if self.mod_time() {
                    print!("/");
                }
                print!("modified");
            }
        }
        if self.props() {
            print!("\tprops");
        }
        println!();
    }

    async fn print_directory_iter(&self, ots: &OtsClient) -> Result<()> {
        ots.first().await?;
        for index in 0.. {
            print!("{index}");
            if self.id() {
                if let Some(id) = ots.id().await? {
                    print!("\t{}", id);
                }
            }
            if self.name() {
                print!("\t{:?}", ots.name().await?);
            }
            if self.type_() {
                print!("\t{}", ots.type_().await?);
            }
            if self.any_size() {
                let size = ots.size().await?;
                print!("\t");
                if self.cur_size() {
                    print!("{}", size.current);
                }
                if self.alloc_size() {
                    if self.cur_size() {
                        print!("/");
                    }
                    print!("{}", size.allocated);
                }
            }
            if self.any_time() {
                print!("\t");
                if self.crt_time() {
                    if let Some(dt) = ots.first_created().await? {
                        print!("{dt}");
                    }
                }
                if self.mod_time() {
                    if let Some(dt) = ots.last_modified().await? {
                        if self.crt_time() {
                            print!("/");
                        }
                        print!("{dt}");
                    }
                }
            }
            if self.props() {
                print!("\t{}", ots.properties().await?);
            }
            println!();

            if !ots.next().await? {
                break;
            }
        }
        Ok(())
    }

    async fn print_directory_data(
        &self,
        data: &[u8],
        _ots: &bluez_async_ots::OtsClient,
    ) -> Result<()> {
        //println!("{}", HexDump(data));

        for (index, ent) in DirEntries::from(data).enumerate() {
            let meta = ent?;

            /*
            if self.cur_size() && meta.current_size.is_none()
                || self.alloc_size() && meta.allocated_size.is_none()
            {
                // try fill sizes from meta
                if let Some(id) = meta.id {
                    ots.go_to(id).await?;
                }
                let size = ots.size().await?;
                if meta.current_size.is_none() {
                    meta.current_size = size.current.into();
                }
                if meta.allocated_size.is_none() {
                    meta.allocated_size = size.allocated.into();
                }
            }
            */

            print!("{index}");
            if self.id() {
                if let Some(id) = &meta.id {
                    print!("\t{}", id);
                }
            }
            if self.name() {
                print!("\t{:?}", meta.name);
            }
            if self.type_() {
                print!("\t{}", meta.type_);
            }
            if self.any_size() {
                print!("\t");
                if self.cur_size() && meta.current_size.is_some() {
                    print!("{}", meta.current_size.unwrap());
                }
                if self.alloc_size() && meta.allocated_size.is_some() {
                    if self.cur_size() {
                        print!("/");
                    }
                    print!("{}", meta.allocated_size.unwrap());
                }
            }
            if self.any_time() {
                print!("\t");
            }
            if self.props() {
                print!("\t{}", meta.properties);
            }
            println!();
        }
        Ok(())
    }
}

impl cli::ReadArgs {
    pub async fn run(&self, ots: &OtsClient) -> Result<()> {
        self.object.select(ots).await?;

        let data = ots.read(self.range.offset, self.range.length).await?;

        if let Some(file) = &self.file {
            tokio::fs::write(file, data).await?;
        } else {
            print!("{}", HexDump(&data));
        }

        Ok(())
    }
}

impl cli::WriteArgs {
    pub async fn run(&self, ots: &OtsClient) -> Result<()> {
        self.object.select(ots).await?;

        let data = if let Some(file) = &self.file {
            if file == std::path::Path::new("-") {
                // read from stdin
                let mut data = Vec::new();
                tokio::io::stdin().read_to_end(&mut data).await?;
                data
            } else {
                // read from file
                tokio::fs::read(file).await?
            }
        } else {
            // read from stdin
            let mut data = Vec::new();
            tokio::io::stdin().read_to_end(&mut data).await?;
            let chars = core::str::from_utf8(&data)?.chars();
            let mut data = Vec::with_capacity(data.len() / 2);
            let mut half = None;
            for chr in chars {
                if chr.is_whitespace() {
                    // skip spaces
                    continue;
                }
                if let Some(dig) = chr.to_digit(16) {
                    let dig = dig as u8;
                    if let Some(half) = half.take() {
                        data.push(half | dig);
                    } else {
                        half = Some(dig << 4);
                    }
                } else {
                    return Err(Error::HexError);
                }
            }
            data
        };

        let data = if let Some(len) = self.range.length {
            if data.len() > len {
                eprintln!(
                    "Input data will be truncated to {len} bytes because length argument used"
                );
                &data[..len]
            } else {
                &data[..]
            }
        } else {
            &data[..]
        };

        log::trace!("{}", HexDump(data));

        let mode = if self.truncate {
            bluez_async_ots::WriteMode::Truncate
        } else {
            bluez_async_ots::WriteMode::default()
        };

        ots.write(self.range.offset, data, mode).await?;

        Ok(())
    }
}

impl cli::ObjSel {
    pub async fn select(&self, ots: &OtsClient) -> Result<()> {
        if let Some(req_index) = self.index {
            ots.first().await?;
            for _ in 0..req_index {
                if !ots.next().await? {
                    return Err(Error::NoObject);
                }
            }
        } else if let Some(req_id) = self.id {
            if !ots.go_to(req_id).await? {
                return Err(Error::ObjIdError);
            }
        } else if let Some(req_name) = &self.name {
            ots.first().await?;
            loop {
                if &ots.name().await? == req_name {
                    break;
                }
                if !ots.next().await? {
                    return Err(Error::NoObject);
                }
            }
        } else {
            return Err(Error::ObjIdError);
        }
        Ok(())
    }
}

struct HexDump<T>(T);

impl<T: AsRef<[u8]>> core::fmt::Display for HexDump<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        hex_pp::pretty_hex_write(f, &self.0)
    }
}
