use anyhow::Result;
use esp_idf_hal::delay::Delay;
use esp_idf_hal::gpio::PinDriver;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::*;
use esp_idf_svc::log::EspLogger;
use log::*;
#[cfg(any(board = "m5atom", board = "m5stamp"))]
use smart_leds::SmartLedsWrite;
use std::io::stdin;
use std::sync::mpsc;
use std::thread;

#[cfg(any(board = "m5atom", board = "m5stamp"))]
use ws2812_esp32_rmt_driver::driver::color::LedPixelColorGrbw32;
#[cfg(any(board = "m5atom", board = "m5stamp"))]
use ws2812_esp32_rmt_driver::{LedPixelEsp32Rmt, RGB8};

mod morse;
mod nvskey;
mod param;
mod server;
mod wifi;

use crate::morse::Morse;
use crate::nvskey::NVSkey;
use crate::param::KeyerParam;
use crate::server::spawn_server;
use crate::wifi::WiFiConnection;

#[toml_cfg::toml_config]
pub struct Config {
    #[default("ActPaddle")]
    ap_ssid: &'static str,
    #[default("actpaddle")]
    ap_pass: &'static str,
    #[default("")]
    stn_ssid: &'static str,
    #[default("")]
    stn_pass: &'static str,
    #[default("actpaddle")]
    hostname: &'static str,
}

#[cfg(target_arch = "riscv32")]
extern "C" {
    fn init_usb();
}

#[cfg(target_arch = "xtensa")]
fn init_uart0() {
    use core::ptr::null_mut;
    use esp_idf_sys::{esp_vfs_dev_uart_use_driver, uart_driver_install};
    unsafe {
        esp_idf_sys::esp!(uart_driver_install(0, 256, 0, 0, null_mut(), 0))
            .expect("unable to initialize UART0 driver");
        esp_vfs_dev_uart_use_driver(0);
    }
}

fn patches() {
    esp_idf_sys::link_patches();
    #[cfg(target_arch = "xtensa")]
    init_uart0();
    #[cfg(target_arch = "riscv32")]
    unsafe {
        init_usb();
    }
}

static LOGGER: EspLogger = EspLogger;

fn main() -> Result<()> {
    patches();

    log::set_logger(&LOGGER).map(|()| LOGGER.initialize())?;
    LOGGER.set_target_level("wifi", log::LevelFilter::Off)?;

    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;

    #[cfg(board = "m5atom")]
    let di = PinDriver::output(peripherals.pins.gpio33)?;
    #[cfg(board = "m5atom")]
    let dah = PinDriver::output(peripherals.pins.gpio23)?;
    #[cfg(board = "m5atom")]
    const LED_PIN: u32 = 27;

    #[cfg(board = "m5stamp")]
    let di = PinDriver::output(peripherals.pins.gpio4)?;
    #[cfg(board = "m5stamp")]
    let dah = PinDriver::output(peripherals.pins.gpio3)?;
    #[cfg(board = "m5stamp")]
    const LED_PIN: u32 = 2;

    #[cfg(any(board = "m5atom", board = "m5stamp"))]
    let mut led = LedPixelEsp32Rmt::<RGB8, LedPixelColorGrbw32>::new(0, LED_PIN).unwrap();
    #[cfg(any(board = "m5atom", board = "m5stamp"))]
    let empty_color = std::iter::repeat(RGB8::default()).take(1);
    #[cfg(any(board = "m5atom", board = "m5stamp"))]
    let white_color = std::iter::repeat(RGB8 {
        r: 10,
        g: 10,
        b: 10,
    })
    .take(1);
    #[cfg(any(board = "m5atom", board = "m5stamp"))]
    let red_color = std::iter::repeat(RGB8 {
        r: 20,
        g: 0,
        b: 0,
    })
    .take(1);

    #[cfg(board = "xiao-esp32c3")]
    let di = PinDriver::output(peripherals.pins.gpio3)?;
    #[cfg(board = "xiao-esp32c3")]
    let dah = PinDriver::output(peripherals.pins.gpio2)?;
    #[cfg(board = "xiao-esp32c3")]
    let mut led = PinDriver::output(peripherals.pins.gpio4)?;

    let mut nvs = NVSkey::new("actpaddle")?;

    let mut morse = Morse::new(di, dah);

    let (tx_web, rx_web) = mpsc::channel::<KeyerParam>();
    let (tx_web2, rx_web2) = mpsc::channel::<KeyerParam>();
    let (tx_serial, rx_serial) = mpsc::channel::<KeyerParam>();

    if !CONFIG.stn_ssid.is_empty() {
        nvs.clear()?;
        nvs.set_ssid(CONFIG.stn_ssid, CONFIG.stn_pass)?;
    }

    let saved_ap = nvs.get_ssid_list();
    let mut wifi = WiFiConnection::new(
        peripherals.modem,
        sysloop.clone(),
        CONFIG.hostname,
        CONFIG.ap_ssid,
        CONFIG.ap_pass,
    )?;
    
    #[cfg(any(board = "m5atom", board = "m5stamp"))]
    led.write(red_color.clone())?;

    wifi.wifi_start(None, saved_ap)?;

    let _server = spawn_server(tx_web, rx_web2);

    thread::spawn(move || {
        let reader = stdin();
        loop {
            let mut line = String::new();
            if let Err(e) = reader.read_line(&mut line) {
                info!("Error: {e}");
            } else {
                let mesg: Result<KeyerParam, serde_json::Error> = serde_json::from_str(&line);
                if let Ok(mesg) = mesg {
                    info!("STDIN= {:?}", mesg);
                    let _ = tx_serial.send(mesg);
                } else {
                    info!("JSONError: {:?}", mesg);
                }
            }
            Delay::delay_ms(100);
        }
    });

    loop {
        if let Ok(msg) = rx_web.try_recv() {
            if msg.ssid.is_some() || msg.del_ssid.is_some() || msg.ssidlist.is_some() {
                if let Some(ssid) = msg.ssid {
                    if let Some(password) = msg.password {
                        nvs.set_ssid(&ssid[2..], &password)?;
                        info!("Set New SSID = {:?}", &ssid[2..]);
                        let saved_ap = nvs.get_ssid_list();
                        wifi.wifi_start(Some(&ssid[2..]), saved_ap)?;
                    }
                }
                if let Some(ssid) = msg.del_ssid {
                    nvs.del_ssid(&ssid[2..])?;
                    info!("Delete SSID = {:?}", &ssid[2..]);
                }
                if msg.ssidlist.is_some() {
                    let mut k = KeyerParam::default();
                    let saved_ap = nvs.get_ssid_list();
                    if let Some(ssids) = wifi.scanned_ap_list(saved_ap) {
                        k.ssidlist = Some(ssids.to_vec());
                    }
                    tx_web2.send(k)?;
                }
            } else {
                morse.interp(&msg);
            }
        }

        if let Ok(msg) = rx_serial.try_recv() {
            morse.interp(&msg);
        }

        wifi.wifi_loop()?;

        if wifi.is_up() {
            #[cfg(any(board = "m5atom", board = "m5stamp"))]
            led.write(empty_color.clone())?;
            #[cfg(board = "xiao-esp32c3")]
            led.set_high()?;
        } else {
            #[cfg(any(board = "m5atom", board = "m5stamp"))]
            led.write(white_color.clone())?;
            #[cfg(board = "xiao-esp32c3")]
            led.set_low()?;
        }
        Delay::delay_ms(100);
    }
}
