use anyhow::{bail, Result};
use esp_idf_svc::nvs::*;
use log::info;

pub struct NVSkey {
    nvs: EspNvs<NvsDefault>,
}

impl NVSkey {
    pub fn new(namespace: &str) -> Result<NVSkey> {
        let nvs_default_partition: EspNvsPartition<NvsDefault> = EspDefaultNvsPartition::take()?;
        let nvs = match EspNvs::new(nvs_default_partition, namespace, true) {
            Ok(nvs) => {
                nvs
            }
            Err(e) => panic!("Could't get namespace {:?}", e),
        };
        Ok(NVSkey { nvs })
    }

    pub fn get_value<'a>(&mut self, key: &'a str, default: &'a str) -> Result<String> {
        const MAX_STR_LEN: usize = 256;
        if let Some(len) = self.nvs.str_len(key)? {
            let mut buffer: [u8; MAX_STR_LEN] = [0; MAX_STR_LEN];
            if let Some(v) = self.nvs.get_str(key, &mut buffer)? {
                let mut v = v.to_string();
                v.pop();
                Ok(v)
            } else {
                bail!("Can't read from nvs key={:?}", key);
            }
        } else {
            self.nvs.set_str(key, default)?;
            Ok(default.to_string())
        }
    }

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        self.nvs.set_str(key, value)?;
        Ok(())
    }
}
