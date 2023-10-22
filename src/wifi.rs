use anyhow::Result;
use embedded_svc::wifi::*;
use esp_idf_hal::peripheral;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::mdns::EspMdns;
use esp_idf_svc::wifi::*;
use log::*;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WiFiState {
    Idle,
    Halt,
    Started,
    Connected,
    IfUpClient,
    IfUpAp,
}

pub struct WiFiConnection<'a, F1, F2>
where
    F1: FnMut(WiFiState) -> Option<WiFiState>,
    F2: FnMut(WiFiState) -> Option<WiFiState>,
{
    esp_wifi: BlockingWifi<EspWifi<'static>>,
    mdns: EspMdns,
    host: &'a str,
    ap_ssid: &'a str,
    ap_pass: &'a str,
    saved_ap_list: Option<HashMap<String, String>>,
    scanned_ap_list: Option<Vec<String>>,
    state: WiFiState,
    is_ap: bool,
    event_handler: Option<F1>,
    periodical_handler: Option<F2>,
}

impl<'a, F1, F2> WiFiConnection<'a, F1, F2>
where
    F1: FnMut(WiFiState) -> Option<WiFiState>,
    F2: FnMut(WiFiState) -> Option<WiFiState>,
{
    pub fn new(
        modem: impl peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
        sysloop: EspSystemEventLoop,
        host: &'a str,
        ap_ssid: &'a str,
        ap_pass: &'a str,
    ) -> Result<Self> {
        let nvs = None;
        let esp_wifi = BlockingWifi::wrap(
            EspWifi::<'static>::new(modem, sysloop.clone(), nvs)?,
            sysloop,
        )?;
        let mdns = EspMdns::take()?;
        Ok(Self {
            esp_wifi,
            mdns,
            host,
            ap_ssid,
            ap_pass,
            saved_ap_list: None,
            scanned_ap_list: None,
            state: WiFiState::Idle,
            is_ap: false,
            event_handler: None,
            periodical_handler: None,
        })
    }

    pub fn wifi_start(
        &mut self,
        default_ssid: Option<&str>,
        saved_ap_list: Option<HashMap<String, String>>,
    ) -> Result<()> {
        self.saved_ap_list = saved_ap_list;

        let _ = self.esp_wifi.disconnect();
        let _ = self.esp_wifi.stop();

        self.state = WiFiState::Idle;
        self.wifi_loop()?;

        self.esp_wifi
            .set_configuration(&Configuration::Client(ClientConfiguration::default()))?;
        self.esp_wifi.start()?;

        let ap_info = self.ap_scan();
        let candidate = ap_info.iter().find(|&a| {
            if let Some(ssid) = default_ssid {
                a.ssid == ssid
            } else {
                self.saved_ap_list.is_some()
                    && self
                        .saved_ap_list
                        .as_ref()
                        .unwrap()
                        .contains_key(&a.ssid.to_string())
            }
        });

        let ap_conf = AccessPointConfiguration {
            ssid: self.ap_ssid.into(),
            ssid_hidden: false,
            password: self.ap_pass.into(),
            auth_method: AuthMethod::WPA2Personal,
            ..Default::default()
        };

        let mut stn_conf = ClientConfiguration::default();

        if let Some(ap) = candidate {
            let ssid = ap.ssid.as_str();
            let passwd = self
                .saved_ap_list
                .as_ref()
                .unwrap()
                .get(ssid)
                .unwrap()
                .as_str();
            stn_conf = ClientConfiguration {
                ssid: ssid.into(),
                password: passwd.into(),
                channel: Some(ap.channel),
                ..Default::default()
            }
        };

        let ap_config = Configuration::AccessPoint(ap_conf.clone());
        let client_config = Configuration::Client(stn_conf.clone());

        if stn_conf.ssid.is_empty() || stn_conf.password.is_empty() {
            self.esp_wifi.set_configuration(&ap_config).unwrap();
            self.is_ap = true;
        } else {
            self.esp_wifi.set_configuration(&client_config).unwrap();
            self.is_ap = false;
        }

        self.esp_wifi.start()?;
        self.wifi_loop()?;

        let mut rc = 0;
        while self.esp_wifi.connect().is_err() {
            rc += 1;
            info!("Retry connecting {}", rc);
            if rc > 2 {
                info!("Restart AP as {:?}", ap_config);
                self.esp_wifi.stop()?;
                self.esp_wifi.set_configuration(&ap_config)?;
                self.esp_wifi.start()?;
                break;
            }
        }

        self.mdns.set_hostname(self.host)?;
        info!("Staring WiFi.");
        self.wifi_loop()?;
        Ok(())
    }

    pub fn wifi_stop(&mut self) -> Result<()> {
        self.esp_wifi.disconnect()?;
        self.esp_wifi.stop()?;
        Ok(())
    }

    pub fn ap_scan(&mut self) -> Vec<AccessPointInfo> {
        let mut ap_info = self.esp_wifi.scan().unwrap();
        ap_info.sort_by(|a, b| b.signal_strength.cmp(&a.signal_strength));
        let aplist = ap_info.iter().map(|ap| ap.ssid.to_string()).collect();
        self.scanned_ap_list = if !ap_info.is_empty() {
            Some(aplist)
        } else {
            None
        };
        ap_info
    }

    pub fn scanned_ap_list(
        &mut self,
        saved_ap_list: Option<HashMap<String, String>>,
    ) -> Option<Vec<String>> {
        self.saved_ap_list = saved_ap_list;
        if self.scanned_ap_list.is_some() {
            let res = self
                .scanned_ap_list
                .as_ref()
                .unwrap()
                .iter()
                .map(|ap| {
                    if self.saved_ap_list.is_some()
                        && self.saved_ap_list.as_ref().unwrap().contains_key(ap)
                    {
                        format!("o {}", ap)
                    } else {
                        format!("x {}", ap)
                    }
                })
                .collect();
            Some(res)
        } else {
            None
        }
    }

    pub fn add_event_handler(&mut self, f: F1) {
        self.event_handler = Some(f);
    }

    pub fn add_periodical_handler(&mut self, f: F2) {
        self.periodical_handler = Some(f);
    }

    pub fn wifi_loop(&mut self) -> Result<WiFiState> {
        let prev = self.state;

        if let Ok(started) = self.esp_wifi.is_started() {
            match started {
                true => {
                    self.state = WiFiState::Started;
                    if let Ok(connected) = self.esp_wifi.is_connected() {
                        match connected {
                            true => {
                                self.state = WiFiState::Connected;
                                if let Ok(isup) = self.esp_wifi.is_up() {
                                    match isup {
                                        true => {
                                            self.state = if self.is_ap {
                                                WiFiState::IfUpAp
                                            } else {
                                                WiFiState::IfUpClient
                                            }
                                        }
                                        false => {}
                                    }
                                }
                            }
                            false => {}
                        }
                    }
                }
                false => self.state = WiFiState::Halt,
            }
        }

        #[allow(clippy::single_match, clippy::collapsible_match)]
        if prev != self.state {
            match self.state {
                WiFiState::Started => {}
                WiFiState::IfUpAp => {
                    let ap_info = self.esp_wifi.wifi().ap_netif().get_ip_info();

                    info!("AP Info: {:?}", ap_info);
                }
                WiFiState::IfUpClient => {
                    let sta_info = self.esp_wifi.wifi().sta_netif().get_ip_info();
                    info!("STN Info: {:?}", sta_info);
                }
                _ => {}
            }
            if self.event_handler.is_some() {
                if let Some(next_state) = self.event_handler.as_mut().unwrap()(self.state) {
                    match next_state {
                        WiFiState::Halt => (),
                        _ => (),
                    };
                }
            }
        } else {
            match self.state {
                WiFiState::Started => {}
                _ => (),
            }
            if self.periodical_handler.is_some() {
                if let Some(next_state) = self.periodical_handler.as_mut().unwrap()(self.state) {
                    match next_state {
                        WiFiState::Halt => self.wifi_stop()?,
                        _ => (),
                    };
                }
            }
        }
        info!("State={:?}", self.state);
        Ok(self.state)
    }
}
