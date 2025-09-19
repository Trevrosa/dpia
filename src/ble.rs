use trouble_host::prelude::{
    DefaultPacketPool, FromGatt, Runner, appearance, descriptors, gatt_server, gatt_service,
    service,
};
use trouble_host::{
    Address, Controller, Host, HostResources, PacketPool,
    gap::{GapConfig, PeripheralConfig},
};

const MAX_CONNECTIONS: usize = 2;

const L2CAP_CHANNELS_MAX: usize = 2;

#[gatt_server]
struct Server {
    temp_service: TemperatureService,
}

#[gatt_service(uuid = service::ENVIRONMENTAL_SENSING)]
struct TemperatureService {
    #[descriptor(uuid = descriptors::VALID_RANGE, read, value = [-30, 100])]
    level: i8,
}

async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(err) = runner.run().await {
            let err = defmt::Debug2Format(&err);
            panic!("[ble_task] error: {err:?}");
        }
    }
}

pub async fn peripheral(controller: impl Controller, address: [u8; 6]) {
    let mut resources: HostResources<DefaultPacketPool, MAX_CONNECTIONS, L2CAP_CHANNELS_MAX> =
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
}
