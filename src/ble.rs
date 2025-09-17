use trouble_host::{Address, Controller, HostResources, prelude::DefaultPacketPool};

const MAX_CONNECTIONS: usize = 2;

const L2CAP_CHANNELS_MAX: usize = 2;

pub fn peripheral(controller: impl Controller, address: [u8; 6]) {
    let mut resources: HostResources<DefaultPacketPool, MAX_CONNECTIONS, L2CAP_CHANNELS_MAX> =
        HostResources::new();
}
