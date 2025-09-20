use defmt::unwrap;
use embassy_futures::join;
use trouble_host::BleHostError;
use trouble_host::gatt::GattConnection;
use trouble_host::prelude::{
    AdStructure, BR_EDR_NOT_SUPPORTED, DefaultPacketPool, FromGatt, LE_GENERAL_DISCOVERABLE,
    Peripheral, Runner, appearance, descriptors, gatt_server, gatt_service, service,
};
use trouble_host::{
    Address, Controller, Host, HostResources, PacketPool,
    gap::{GapConfig, PeripheralConfig},
};

const MAX_CONNECTIONS: usize = 2;

const MAX_L2CAP_CHANNELS: usize = 2;

#[gatt_server]
struct Server {
    temp_service: TemperatureService,
}

#[gatt_service(uuid = service::ENVIRONMENTAL_SENSING)]
struct TemperatureService {
    #[descriptor(uuid = descriptors::VALID_RANGE, read, value = [-30, 100])]
    level: i8,
}

pub async fn peripheral(controller: impl Controller, address: [u8; 6]) {
    let mut resources: HostResources<DefaultPacketPool, MAX_CONNECTIONS, MAX_L2CAP_CHANNELS> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources);
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

    let advertiser = peripheral.advertise(
        &Default::default(),
        trouble_host::prelude::Advertisement::ConnectableScannableUndirected {
            adv_data: &advertiser_data[..len],
            scan_data: &[],
        },
    ).await?;

    defmt::info!("advertising");
    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    defmt::info!("ble connection established");

    Ok(conn)
}
