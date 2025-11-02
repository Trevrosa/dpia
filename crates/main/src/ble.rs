use defmt::unwrap;
use embassy_futures::join;
use embassy_time::{Duration, Timer};
use trouble_host::prelude::Advertisement::NonconnectableNonscannableUndirected;
use trouble_host::prelude::{
    AdStructure, AdvertisementParameters, BR_EDR_NOT_SUPPORTED, DefaultPacketPool,
    LE_GENERAL_DISCOVERABLE,
};
use trouble_host::{Address, Controller, Host, HostResources};

// TODO: this actually should be ble beacon or something else

pub async fn beacon(controller: impl Controller, address: [u8; 6]) {
    let mut resources: HostResources<DefaultPacketPool, 0, 0, 27> = HostResources::new();
    let address = Address::random(address);
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host {
        mut peripheral,
        mut runner,
        ..
    } = stack.build();

    defmt::info!("built ble host");

    let mut adv_data = [0; 64];
    let adv_len = AdStructure::encode_slice(
        &[
            AdStructure::CompleteLocalName(b"Trevor's IA"),
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
        ],
        &mut adv_data[..],
    );
    let adv_len = unwrap!(adv_len);
    let adv_data = &adv_data[..adv_len];

    defmt::info!("advertising");

    let _ = join::join(runner.run(), async {
        loop {
            let mut params = AdvertisementParameters::default();
            params.interval_min = Duration::from_millis(25);
            params.interval_max = Duration::from_millis(150);
            let _advertiser = peripheral
                .advertise(&params, NonconnectableNonscannableUndirected { adv_data })
                .await
                .unwrap();
            loop {
                Timer::after(ADVERTISE_).await;
            }
        }
    });
}
