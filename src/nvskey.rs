use anyhow::{bail, Result};
use esp_idf_svc::nvs::*;

pub struct NVSkey {
    nvs: EspNvs<NvsDefault>,
    max_keys: u8,
}

impl NVSkey {
    pub fn new(namespace: &str) -> Result<NVSkey> {
        let nvs_default_partition: EspNvsPartition<NvsDefault> = EspDefaultNvsPartition::take()?;
        let nvs = match EspNvs::new(nvs_default_partition, namespace, true) {
            Ok(nvs) => nvs,
            Err(e) => panic!("Could't get namespace {:?}", e),
        };
        Ok(NVSkey { nvs, max_keys: 4 })
    }

    pub fn get_ssid_list(&mut self) -> Option<Vec<(String, String)>> {
        let mut result = Vec::<(String, String)>::new();
        for n in 0..self.max_keys {
            let ssidkey = format!("ssid{}", n);
            let passwdkey = format!("passwd{}", n);
            if let Some(ssid) = self.get_value(&ssidkey) {
                result.push((ssid, self.get_value(&passwdkey).unwrap()));
            }
        }
        if result.len() > 0 {
            Some(result)
        } else {
            None
        }
    }

    pub fn set_ssid_passwd(&mut self, new_ssid: &str, new_passwd: &str) -> Result<()> {
        for n in 0..self.max_keys {
            let ssidkey = format!("ssid{}", n);
            let passwdkey = format!("passwd{}", n);
            if let Some(ssid) = self.get_value(&ssidkey) {
                if ssid == new_ssid {
                    if !new_passwd.is_empty() {
                        self.nvs.set_str(&passwdkey, new_passwd);
                    }
                    return Ok(());
                }
            } else {
                self.nvs.set_str(&ssidkey, new_ssid);
                self.nvs.set_str(&passwdkey, new_passwd);
                return Ok(());
            }
        }
        let n = self.max_keys - 1;
        let ssidkey = format!("ssid{}", n);
        let passwdkey = format!("passwd{}", n);
        self.nvs.set_str(&ssidkey, new_ssid);
        self.nvs.set_str(&passwdkey, new_passwd);
        Ok(())
    }

    pub fn get_value(&self, key: &str) -> Option<String> {
        const MAX_STR_LEN: usize = 256;
        if self.nvs.str_len(key).unwrap().is_some() {
            let mut buffer: [u8; MAX_STR_LEN] = [0; MAX_STR_LEN];
            if let Some(v) = self.nvs.get_str(key, &mut buffer).unwrap() {
                let mut v = v.to_string();
                v.pop();
                Some(v)
            } else {
                None
            }
        } else {
            None
        }
    }
}
