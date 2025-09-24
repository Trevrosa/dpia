use defmt::unwrap;
use embassy_futures::join;
use trouble_host::BleHostError;
use trouble_host::gatt::{GattConnection, GattConnectionEvent, GattEvent};
use trouble_host::prelude::{
    AdStructure, AdvertisementParameters, BR_EDR_NOT_SUPPORTED, DefaultPacketPool, FromGatt,
    LE_GENERAL_DISCOVERABLE, Peripheral, Runner, appearance, characteristic, gatt_server,
    gatt_service, service,
};
use trouble_host::{
    Address, Controller, Host, HostResources, PacketPool,
    gap::{GapConfig, PeripheralConfig},
};

const MAX_CONNECTIONS: usize = 2;

const MAX_L2CAP_CHANNELS: usize = 2;

#[gatt_server]
struct Server {
    sense_service: SenseService,
}

#[gatt_service(uuid = service::ENVIRONMENTAL_SENSING)]
struct SenseService {
    #[characteristic(uuid = characteristic::TEMPERATURE_MEASUREMENT, read, notify)]
    ground_temp: i8,
    #[characteristic(uuid = characteristic::TEMPERATURE_MEASUREMENT, read, notify)]
    air_temp: i8,
    #[characteristic(uuid = characteristic::TEMPERATURE_MEASUREMENT, read, notify)]
    humidity: u8,
}

pub async fn peripheral(controller: impl Controller, address: [u8; 6]) {
    let mut resources: HostResources<DefaultPacketPool, MAX_CONNECTIONS, MAX_L2CAP_CHANNELS> =
        HostResources::new();
    let address = Address::random(address);
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host {
        mut peripheral,
        runner,
        ..
    } = stack.build();

    defmt::info!("starting advertising service");

    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "Trevor's ia",
        appearance: &appearance::sensor::TEMPERATURE_SENSOR,
    }));
    let server = unwrap!(server);

    let mut adv_loop = async || {
        loop {
            match advertise("Trevor test", &mut peripheral, &server).await {
                Ok(conn) => {
                    let a = gatt_event_task(&conn).await;
                    // let b = custom_task(&server, &conn, &stack);
                    // run until any task ends (usually because the connection has been closed),
                    // then return to advertising state.
                    // select(a, b).await;

                    if let Err(err) = a {
                        defmt::error!("[gatt] gatt event error: {}", err);
                    }
                }
                Err(err) => {
                    let err = defmt::Debug2Format(&err);
                    panic!("advertise error: {err:?}");
                }
            }
        }
    };

    join::join(ble_task(runner), adv_loop()).await;
}

async fn gatt_event_task<P: PacketPool>(
    /*server: &Server<'_>, */ conn: &GattConnection<'_, '_, P>,
) -> Result<(), trouble_host::Error> {
    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                match &event {
                    GattEvent::Read(event) => {
                        // TODO:
                        // if event.handle() == level.handle {
                        // let value = server.get(&level);
                        defmt::info!("[gatt] read to handle `{}`", event.handle());
                    }
                    GattEvent::Write(event) => {
                        defmt::info!(
                            "[gatt] write to handle `{}` with {:?}",
                            event.handle(),
                            event.data()
                        );
                    }
                    GattEvent::Other(_) => {}
                }
                // This step is also performed at drop(), but writing it explicitly is necessary
                // in order to ensure reply is sent.
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => defmt::warn!("[gatt] error sending response: {:?}", e),
                }
            }
            _ => {} // ignore other Gatt Connection Events
        }
    };

    defmt::info!("[gatt] disconnected: {:?}", reason);
    Ok(())
}

async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(err) = runner.run().await {
            let err = defmt::Debug2Format(&err);
            panic!("[ble_task] error: {err:?}");
        }
    }
}

// FIXME: can we move constructing the advertiser out of this fn?
async fn advertise<'value, 'server, C: Controller>(
    name: &'value str,
    peripheral: &mut Peripheral<'value, C, DefaultPacketPool>,
    server: &'server Server<'value>,
) -> Result<GattConnection<'value, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];

    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[[0x0f, 0x18]]),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;

    let advertiser = peripheral
        .advertise(
            &AdvertisementParameters::default(),
            trouble_host::prelude::Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &[],
            },
        )
        .await?;

    defmt::info!("advertising");
    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    defmt::info!("ble connection established");

    Ok(conn)
}
