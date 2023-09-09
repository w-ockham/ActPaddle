use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::PinDriver;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::*;
use esp_idf_svc::log::EspLogger;
use log::*;
use std::io::stdin;
use std::sync::mpsc;
use std::thread;

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

fn main() -> anyhow::Result<()> {
    patches();

    log::set_logger(&LOGGER).map(|()| LOGGER.initialize())?;
    LOGGER.set_target_level("wifi", log::LevelFilter::Off)?;

    let peripherals = Peripherals::take().unwrap();

    let sysloop = EspSystemEventLoop::take()?;

    let di = PinDriver::output(peripherals.pins.gpio3)?;
    let dah = PinDriver::output(peripherals.pins.gpio2)?;

    let mut morse = Morse::new(di, dah);

    let (tx_web, rx_web) = mpsc::channel::<KeyerParam>();
    let (tx_web2, rx_web2) = mpsc::channel::<KeyerParam>();
    let (tx_serial, rx_serial) = mpsc::channel::<KeyerParam>();


    let mut nvs = NVSkey::new("actpaddle")?;

    let stn_ssid = nvs.get_value("default_ssid", CONFIG.stn_ssid).unwrap();
    let stn_passwd = nvs.get_value("default_passwd", CONFIG.stn_pass).unwrap();
    
    let mut wifi = WiFiConnection::new(peripherals.modem, sysloop.clone()).unwrap();
    
    wifi.wifi_start(
        CONFIG.hostname,
        &stn_ssid,
        &stn_passwd,
        CONFIG.ap_ssid,
        CONFIG.ap_pass,
    ).unwrap();

    let _server = spawn_server(tx_web, rx_web2);

    thread::spawn(move || {
        let reader = stdin();
        loop {
            let mut line = String::new();
            if let Err(e) = reader.read_line(&mut line) {
                print!("Error: {e}\r\n");
            } else {
                let mesg: Result<KeyerParam, serde_json::Error> = serde_json::from_str(&line);
                if let Ok(mesg) = mesg {
                    let _ = tx_serial.send(mesg);
                } else {
                    print!("JSONError: {:?}", mesg);
                }
            }
            FreeRtos::delay_ms(10);
        }
    });

    loop {
        let mut interp = |param: KeyerParam| {
            if let Some(s) = param.wpm {
                morse.set_wpm(s);
            }

            if let Some(r) = param.ratio {
                morse.set_ratio(r);
            }

            if let Some(s) = param.letter_space {
                morse.set_letter_space(s);
            }

            if let Some(s) = param.word_space {
                morse.set_word_space(s);
            }

            if let Some(r) = param.reverse {
                if r {
                    morse.reverse();
                } else {
                    morse.normal();
                }
            }
            if let Some(m) = param.to_paddle {
                morse.play(true, &m);
            }

            if let Some(m) = param.to_straight {
                morse.play(false, &m);
            }
        };

        if let Ok(msg) = rx_web.try_recv() {
            if msg.ssid.is_some() || msg.ssidlist.is_some() {
                if let Some(ssid) = msg.ssid {
                    if let Some(password) = msg.password {
                        nvs.set_value("default_ssid", &ssid)?;
                        nvs.set_value("default_passwd", &password)?;
                        info!("SSID = {:?}", ssid);
                        info!("Password= {:?}", password);
                        FreeRtos::delay_ms(3000);
                        unsafe { esp_idf_sys::esp_restart() };
                    }
                }
                if msg.ssidlist.is_some() {
                    let mut k = KeyerParam::default();
                    if let Some(ssids) = wifi.get_ssidlist() {
                        k.ssidlist = Some(ssids);
                    }
                    tx_web2.send(k)?;
                }
            } else {
                interp(msg);
            }
        }

        if let Ok(msg) = rx_serial.try_recv() {
            interp(msg);
        }

        wifi.wifi_loop()?;

        FreeRtos::delay_ms(5);
    }
}
