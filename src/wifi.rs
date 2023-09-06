use std::time::Duration;

use anyhow::{bail, Result};

use esp_idf_hal::delay::FreeRtos;
use log::*;

use embedded_svc::ipv4;
use embedded_svc::wifi::*;
use esp_idf_svc::ping;
use esp_idf_svc::wifi::*;

use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};

use esp_idf_hal::peripheral;

pub fn wifi_connect<'a>(
    modem: impl peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,
    ssid: &'a str,
    pass: &'a str,
) -> Result<Box<EspWifi<'static>>> {
    let nvs = EspDefaultNvsPartition::take()?;
    let mut esp_wifi = EspWifi::new(modem, sysloop.clone(), Some(nvs))?;

    //let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;

    /*
        wifi.set_configuration(&Configuration::Client(ClientConfiguration::default()))?;

        info!("Starting wifi...");

        wifi.start()?;

        info!("Scanning...");

        let ap_infos = wifi.scan()?;

        let ours = ap_infos.into_iter().find(|a| a.ssid == ssid);

        let channel = if let Some(ours) = ours {
            info!(
                "Found configured access point {} on channel {}",
                ssid, ours.channel
            );
            Some(ours.channel)
        } else {
            info!(
                "Configured access point {} not found during scanning, will go with unknown channel",
                ssid
            );
            None
        };
    */
    esp_wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: ssid.into(),
        password: pass.into(),
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    }))?;

    esp_wifi.start()?;
    info!("Connecting wifi...");
    /* 
    esp_wifi.connect()?;
    while !esp_wifi.is_connected().unwrap() {
      let config = esp_wifi.get_configuration().unwrap();
      info!("Waiting for station {:?}", config);
    }

    info!("Waiting for DHCP lease...");
    while !esp_wifi.is_up().unwrap() {
      let config = esp_wifi.get_configuration().unwrap();
      info!("Waiting for interfaces {:?}", config);
    }
  
    let ip_info = esp_wifi.sta_netif().get_ip_info()?;
    info!("Wifi DHCP info: {:?}", ip_info);
*/
    Ok(Box::new(esp_wifi))
}
