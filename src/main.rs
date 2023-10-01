use anyhow::Result;
use bluez_async::BluetoothSession;
use core::time::Duration;
use tokio::time::sleep;

mod ots;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let (_, bs) = BluetoothSession::new().await?;

    bs.start_discovery().await?;
    sleep(Duration::from_secs(2)).await;
    bs.stop_discovery().await?;

    let devs = bs.get_devices().await?;

    let dev = devs
        .into_iter()
        .filter(|dev| {
            dev.name
                .as_ref()
                .map(|name| name == "BM010A")
                .unwrap_or(false)
        })
        .next()
        .ok_or_else(|| anyhow::anyhow!("No device found"))?;
    log::debug!("Device: {dev:#?}");

    let connected = dev.connected;

    if !connected {
        bs.connect_with_timeout(&dev.id, Duration::from_secs(2))
            .await?;
    }

    let ots = ots::OtsClient::new(&bs, &dev.id).await?;

    /*
    ots.first().await?;
    loop {
        let id = ots.id().await?;
        let name = ots.name().await?;
        let size = ots.size().await?;
        let type_ = ots.type_().await?;

        println!("Object: @{id} \"{name}\" #{size:?} &{type_:?}");

        if ots.next().await.is_err() {
            break;
        }
    }
    */

    ots.go_to(0).await?;
    let size = ots.size().await?;
    log::debug!("{size:?}");
    log::debug!("read req");
    let sock = ots.read(None, None).await?;
    let mut data = Vec::<u8>::with_capacity(size.current);
    unsafe { data.set_len(size.current) };
    let mut buf = data.as_mut_slice();
    //let mtu = stream.recv_mtu();
    while !buf.is_empty() {
        let len = sock.recv(&mut buf).await?;
        buf = &mut buf[len..];
        log::debug!("read #{len}");
        if len == 0 {
            break;
        }
    }

    println!("{}", data.len());

    if !connected {
        bs.disconnect(&dev.id).await?;
    }

    Ok(())
}
