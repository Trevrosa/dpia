use defmt::{Debug2Format, error, info, warn};
use embassy_sync_072 as embassy_sync;
use trouble_host::{
    BleHostError, Controller, PacketPool,
    advertise::{
        AdStructure, Advertisement, AdvertisementParameters, BR_EDR_NOT_SUPPORTED,
        LE_GENERAL_DISCOVERABLE,
    },
    att::AttErrorCode,
    gatt::{GattConnection, GattConnectionEvent, GattEvent},
    peripheral::Peripheral,
    prelude::{DefaultPacketPool, FromGatt, Runner, descriptors, gatt_server, gatt_service},
};

use crate::GlobalSensorDataMutex;

#[gatt_server]
pub struct Server {
    humidity: HumidityService,
}

#[gatt_service(uuid = "1234")]
pub struct HumidityService {
    #[descriptor(uuid = descriptors::VALID_RANGE, read, value = [0,100])]
    #[characteristic(uuid = "1235", read, notify)]
    relative: u8,
}

pub async fn server_loop<'vals, C: Controller>(
    peri: &mut Peripheral<'vals, C, DefaultPacketPool>,
    server: &Server<'vals>,
    global_data: &'static GlobalSensorDataMutex,
) -> ! {
    loop {
        let conn = advertise(peri, server).await;
        let conn = match conn {
            Ok(conn) => conn,
            Err(err) => {
                error!("[adv] advertising failed: {}", Debug2Format(&err));
                continue;
            }
        };

        if let Err(err) = events_task(server, &conn, global_data).await {
            error!("[gatt] failure while handling connection: {}", err);
        }
    }
}

async fn events_task(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, DefaultPacketPool>,
    global_data: &'static GlobalSensorDataMutex,
) -> Result<(), trouble_host::Error> {
    let humidity = server.humidity.relative;
    let disconnect_reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                let reply = match event {
                    GattEvent::Read(read) => {
                        if read.handle() == humidity.handle {
                            info!("[gatt] got read to humidity");
                            {
                                let global_data = &*global_data.lock().await;
                                if let Some(val) = global_data.humidity
                                    && let Err(err) = humidity.set(server, &val)
                                {
                                    warn!("[gatt] failed to update humidity from global: {}", err);
                                }
                            }
                            read.accept()
                        } else {
                            warn!("[gatt] got improper read to handle {}", read.handle());
                            read.reject(AttErrorCode::INVALID_HANDLE)
                        }
                    }
                    GattEvent::Write(write) => {
                        info!(
                            "[gatt] got write with data {} handle {}",
                            write.data(),
                            write.handle()
                        );
                        write.reject(AttErrorCode::WRITE_NOT_PERMITTED)
                    }
                    _ => event.reject(AttErrorCode::REQUEST_NOT_SUPPORTED),
                };

                match reply {
                    Ok(reply) => reply.send().await,
                    Err(err) => warn!("[gatt] error sending response: {}", err),
                }
            }
            _ => {}
        }
    };

    info!("[gatt] disconnected: {}", disconnect_reason);

    Ok(())
}

async fn advertise<'vals, 'server, C: Controller>(
    peri: &mut Peripheral<'vals, C, DefaultPacketPool>,
    server: &'server Server<'vals>,
) -> Result<GattConnection<'vals, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut adv_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::CompleteLocalName(b"dpia"),
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
        ],
        &mut adv_data,
    )?;
    let params = AdvertisementParameters::default();
    let advertiser = peri
        .advertise(
            &params,
            Advertisement::ConnectableScannableUndirected {
                adv_data: &adv_data[..len],
                scan_data: &[],
            },
        )
        .await?;

    info!("[adv] advertising");
    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    info!("[adv] connection established");

    Ok(conn)
}

pub async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(err) = runner.run().await {
            defmt::panic!("ble_task error: {}", defmt::Debug2Format(&err));
        }
    }
}
