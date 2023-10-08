use anyhow::Result;
use bluez_async::BluetoothSession;
use bluez_async_ots::{Metadata, OtsClient};
use core::time::Duration;
use either::Either;
use tokio::{io::AsyncReadExt, time::sleep};

mod cli;

#[cfg(feature = "readline")]
mod rl;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let args = <cli::Args as clap::Parser>::parse();

    let (_, bs) = BluetoothSession::new().await?;

    let adapter_id = if let Some(mac_or_name) = &args.adapter {
        Some(
            bs.get_adapters()
                .await?
                .into_iter()
                .filter(
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
                .next()
                .ok_or_else(|| anyhow::anyhow!("No adapter found"))?
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
        .filter(|dev| match (&args.device, &dev.mac_address, &dev.name) {
            (Either::Left(req_addr), dev_addr, _) if req_addr == dev_addr => true,
            (Either::Right(req_name), _, Some(dev_name)) if req_name == dev_name => true,
            _ => false,
        })
        .next()
        .ok_or_else(|| anyhow::anyhow!("No device found"))?;
    log::debug!("Device: {dev:#?}");

    let dev_id = dev.id.clone();
    log::info!("Device: {dev_id:?}");

    let connected = dev.connected;

    if !connected {
        log::info!("Connect to device");
        bs.connect_with_timeout(&dev_id, Duration::from_secs(2))
            .await?;
    }

    let ots = OtsClient::new(&bs, &dev_id).await?;

    use cli::Action::*;
    match args.action {
        List(list) => list.run(&ots).await?,
        Read(read) => read.run(&ots).await?,
        Write(write) => write.run(&ots).await?,
    }

    if !connected {
        log::info!("Disconnect from device");
        bs.disconnect(&dev.id).await?;
    }

    Ok(())
}

impl cli::ListArgs {
    pub async fn run(&self, ots: &OtsClient) -> Result<()> {
        self.print_header();

        // try read special directory object first
        if ots.go_to(0).await.is_ok() {
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
                print!("\t{}", ots.id().await?);
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
            if self.props() {
                print!("\t{:?}", ots.props().await?);
            }
            println!();

            if ots.next().await.is_err() {
                break;
            }
        }
        Ok(())
    }

    async fn print_directory_data(
        &self,
        data: &[u8],
        ots: &bluez_async_ots::OtsClient,
    ) -> Result<()> {
        let mut data = data;
        let mut index = 0;

        //println!("{}", HexDump(data));
        //println!("{:?}", Metadata::split_dir_entry(data));

        while let Some((entry, rest)) = Metadata::split_dir_entry(data)? {
            //println!("{}", HexDump(entry));
            let mut meta = Metadata::try_from(entry)?;

            if self.cur_size() && meta.current_size.is_none()
                || self.alloc_size() && meta.allocated_size.is_none()
            {
                // try fill sizes from meta
                ots.go_to(meta.id).await?;
                let size = ots.size().await?;
                if meta.current_size.is_none() {
                    meta.current_size = size.current.into();
                }
                if meta.allocated_size.is_none() {
                    meta.allocated_size = size.allocated.into();
                }
            }

            print!("{index}");
            if self.id() {
                print!("\t{}", meta.id);
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
            if self.props() {
                print!("\t{:?}", meta.properties);
            }
            println!();

            data = rest;
            index += 1;
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
            #[cfg(feature = "readline")]
            {
                log::trace!("readline begin");
                if let Some(data) = rl::read_hex().await? {
                    log::trace!("readline end: {}", HexDump(&data));
                    data
                } else {
                    return Ok(());
                }
            }

            #[cfg(not(feature = "readline"))]
            {
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
                        anyhow::bail!("Hexadecimal data expected");
                    }
                }
                data
            }
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

        ots.write(self.range.offset, &data, mode).await?;

        Ok(())
    }
}

impl cli::ObjSel {
    pub async fn select(&self, ots: &OtsClient) -> Result<()> {
        if let Some(req_index) = self.index {
            ots.first().await?;
            for _ in 0..req_index {
                ots.next().await?;
            }
        } else if let Some(req_id) = self.id {
            ots.go_to(req_id).await?;
        } else if let Some(req_name) = &self.name {
            ots.first().await?;
            loop {
                if &ots.name().await? == req_name {
                    break;
                }
                ots.next().await?;
            }
        } else {
            anyhow::bail!("Need any of index, id or name to select object to read.");
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
