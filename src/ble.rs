use trouble_host::{Address, Controller, HostResources, prelude::DefaultPacketPool};

const MAX_CONNECTIONS: usize = 2;

const MAX_L2CAP_CHANNELS: usize = 2;

pub fn peripheral(controller: impl Controller, address: [u8; 6]) {
    let mut resources: HostResources<DefaultPacketPool, MAX_CONNECTIONS, MAX_L2CAP_CHANNELS> =
        HostResources::new();
}
