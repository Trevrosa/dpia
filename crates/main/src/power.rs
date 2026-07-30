//! adapted from <https://github.com/antroids/epd-frame-rs-2/> and embassy_rp

use embassy_rp::pac::{
    self,
    common::{RW, Reg},
};

struct Set<T: Copy, F: Fn()>(Reg<T, RW>, T, F);

impl<T: Copy, F: Fn()> Drop for Set<T, F> {
    fn drop(&mut self) {
        self.0.write_value(self.1);
        self.2();
    }
}

fn set_with_post_restore<T: Copy, After: Fn(), F: FnOnce(&mut T) -> After>(
    reg: Reg<T, RW>,
    f: F,
) -> Set<T, impl Fn()> {
    reg.modify(|w| {
        let old = *w;
        let after = f(w);
        Set(reg, old, after)
    })
}

fn set<T: Copy, F: FnOnce(&mut T)>(reg: Reg<T, RW>, f: F) -> Set<T, impl Fn()> {
    set_with_post_restore(reg, |r| {
        f(r);
        || ()
    })
}

macro_rules! set {
    ($w:ident,$val:expr,$($set:ident),+) => {
        $($w.$set($val);)+
    };
}

const POWMAN_PASSWORD: u32 = 0x5AFE << 16;

// FIXME: works but sometimes breaks debug probing after waking

/// An alarm should be set with the aon timer before calling this.
pub fn dormant_sleep() {
    let _set_powman_boot0 = set(pac::POWMAN.boot(0), |w| *w = POWMAN_PASSWORD);
    let _set_powman_boot1 = set(pac::POWMAN.boot(1), |w| *w = POWMAN_PASSWORD);
    let _set_powman_boot2 = set(pac::POWMAN.boot(2), |w| *w = POWMAN_PASSWORD);
    let _set_powman_boot3 = set(pac::POWMAN.boot(3), |w| *w = POWMAN_PASSWORD);

    let _set_powman_dbg_pwrcfg = set(pac::POWMAN.dbg_pwrcfg(), |w| {
        w.set_ignore(true);
        w.0 |= POWMAN_PASSWORD;
    });

    let _set_powman_seq_cfg = set(pac::POWMAN.seq_cfg(), |w| {
        w.set_use_fast_powck(false);
        w.set_run_lposc_in_lp(true);
        w.set_hw_pwrup_sram0(false);
        w.set_hw_pwrup_sram1(false);
        w.0 |= POWMAN_PASSWORD;
    });

    // set_gpio_pull(false);

    // disable all clocks that are not vital in preparation for disabling clock sources.
    let _stop_adc = set(pac::CLOCKS.clk_adc_ctrl(), |w| w.set_enable(false));
    let _stop_usb = set(pac::CLOCKS.clk_usb_ctrl(), |w| w.set_enable(false));
    let _stop_peri = set(pac::CLOCKS.clk_peri_ctrl(), |w| w.set_enable(false));

    let _configure_rosc = (
        set(pac::ROSC.ctrl(), |w| {
            w.set_enable(pac::rosc::vals::Enable::ENABLE);
            w.set_freq_range(pac::rosc::vals::FreqRange::LOW);
        }),
        // div=32
        set(pac::ROSC.div(), |w| w.set_div(pac::rosc::vals::Div(0xaa0))),
    );
    while !pac::ROSC.status().read().stable() {}

    // switch over to rosc as the system clock source. this will change clock sources for
    // watchdog and timer clocks, but timers won't be a concern and the watchdog won't
    // speed up by enough to worry about (unless it's clocked from gpin, which we don't
    // support anyway).
    let _switch_clk_ref = set(pac::CLOCKS.clk_ref_ctrl(), |w| {
        w.set_src(pac::clocks::vals::ClkRefCtrlSrc::LPOSC_CLKSRC);
    });
    let _switch_clk_sys = set(pac::CLOCKS.clk_sys_ctrl(), |w| {
        w.set_src(pac::clocks::vals::ClkSysCtrlSrc::CLKSRC_CLK_SYS_AUX);
        w.set_auxsrc(pac::clocks::vals::ClkSysCtrlAuxsrc::ROSC_CLKSRC);
    });

    let _stop_clk_adc = set(pac::CLOCKS.clk_adc_ctrl(), |w| w.set_enable(false));
    let _stop_clk_usb = set(pac::CLOCKS.clk_usb_ctrl(), |w| w.set_enable(false));

    // oscillator dormancy does not power down plls, we have to do that ourselves. we'll
    // restore them to their prior glory when woken though since the system may be clocked
    // from either (and usb/adc will probably need the USB PLL anyway)
    let _stop_pll_sys = set_with_post_restore(pac::PLL_SYS.pwr(), |w| {
        let wake = !w.pd() && !w.vcopd();
        w.set_pd(true);
        w.set_vcopd(true);
        move || while wake && !pac::PLL_SYS.cs().read().lock() {}
    });
    let _stop_pll_usb = set_with_post_restore(pac::PLL_USB.pwr(), |w| {
        let wake = !w.pd() && !w.vcopd();
        w.set_pd(true);
        w.set_vcopd(true);
        move || while wake && !pac::PLL_USB.cs().read().lock() {}
    });

    let _power_down_xip_cache = set(pac::XIP_CTRL.ctrl(), |w| w.set_power_down(true));

    // dormancy only stops the oscillator we're telling to go dormant, the other remains
    // running. nothing can use xosc at this point any more. not doing this costs an 200µA.
    let _stop_xosc = set_with_post_restore(pac::XOSC.ctrl(), |w| {
        let wake = w.enable() == pac::xosc::vals::Enable::ENABLE;
        if wake {
            w.set_enable(pac::xosc::vals::Enable::DISABLE);
        }
        move || while wake && !pac::XOSC.status().read().stable() {}
    });

    // partially power down the main system memories. we must wait 20 ns before accessing them again
    // let _power_down_srams = set_with_post_restore(pac::SYSCFG.mempowerdown(), |w| {
    //     set!(
    //         set_sram0, set_sram1, set_sram2, set_sram3, set_sram4, set_sram5, set_sram6, set_sram7,
    //         set_sram8, set_sram9, set_usb
    //     );
    //     // ~7 ns per cycle
    //     || cortex_m::asm::delay(10)
    // });

    set_sleepdeep(true);

    pac::ROSC
        .dormant()
        .write_value(pac::rosc::regs::Dormant(0x636f_6d61));

    // waking up after dormant

    pac::ROSC
        .ctrl()
        .modify(|w| w.set_enable(pac::rosc::vals::Enable(0xfab)));
    while !pac::ROSC.status().read().stable() {}

    set_sleepdeep(false);
    // set_gpio_pull(true);

    cortex_m::asm::delay(100);
}

pub fn set_usb_state(enabled: bool) {
    let disabled = !enabled;

    pac::USB.usbphy_direct().modify(|r| {
        set!(
            r,
            disabled,
            set_tx_pd,
            set_rx_pd,
            set_dm_pulldn_en,
            set_dp_pulldn_en
        );
    });

    pac::USB.usbphy_direct_override().modify(|r| {
        set!(
            r,
            disabled,
            set_rx_pd_override_en,
            set_rx_dm_override_en,
            set_rx_dd_override_en,
            set_rx_dp_override_en,
            set_tx_pd_override_en,
            set_tx_dm_override_en,
            set_tx_dm_oe_override_en,
            set_tx_dp_oe_override_en,
            set_tx_diffmode_override_en,
            set_tx_fsslew_override_en,
            set_dm_pulldn_en_override_en,
            set_dp_pulldn_en_override_en
        );
    });
}

// fn set_gpio_pull(enabled: bool) {
//     for gpio_index in 0..48 {
//         pac::PADS_BANK0.gpio(gpio_index).modify(|r| {
//             r.set_pde(enabled);
//             r.set_pue(enabled);
//             r.set_od(!enabled);
//             r.set_ie(enabled);
//         });
//     }
// }

fn set_sleepdeep(enabled: bool) {
    unsafe {
        let mut p = cortex_m::Peripherals::steal();
        if enabled {
            p.SCB.set_sleepdeep();
        } else {
            p.SCB.clear_sleepdeep();
        }
    }
}
