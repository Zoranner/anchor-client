//! 全量硬件快照及其协议哈希。

use anchor_protocol::HardwareInventory;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryReport {
    pub inventory: HardwareInventory,
    pub hash_hex: String,
}

impl InventoryReport {
    pub fn collect() -> Result<Self> {
        let inventory = anchor_system::inventory::collect_hardware_inventory()
            .map_err(anyhow::Error::new)
            .context("采集全量硬件清单失败")?;
        Self::from_inventory(inventory)
    }

    pub fn from_inventory(inventory: HardwareInventory) -> Result<Self> {
        let encoded = serde_json::to_vec(&inventory).context("序列化全量硬件清单失败")?;
        Ok(Self {
            hash_hex: hex::encode(Sha256::digest(encoded)),
            inventory,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn inventory() -> HardwareInventory {
        serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "captured_at": "2026-10-29T08:00:00Z",
            "computer": {
                "manufacturer": "Example",
                "model": "Laptop",
                "serial_number": "SERIAL",
                "bios_vendor": "Example",
                "bios_version": "1.0",
                "bios_release_date": "2026-01-01"
            },
            "operating_system": {
                "name": "Windows 10",
                "version": "10.0",
                "build": "19045",
                "architecture": "x86_64",
                "locale": "zh-CN",
                "hostname": "test-pc"
            },
            "cpu": {
                "manufacturer": "Example",
                "model": "CPU",
                "cores": 4,
                "threads": 8
            },
            "memory_modules": [],
            "disks": [],
            "volumes": [],
            "network_adapters": [],
            "usb_devices": [],
            "optical_drives": [],
            "gpus": []
        }))
        .unwrap()
    }

    #[test]
    fn hash_matches_the_shared_serialization_and_changes_with_content() {
        let mut value = inventory();
        value.captured_at = Utc.with_ymd_and_hms(2026, 10, 29, 8, 0, 0).unwrap();
        let report = InventoryReport::from_inventory(value.clone()).unwrap();
        let expected = hex::encode(Sha256::digest(serde_json::to_vec(&value).unwrap()));
        assert_eq!(report.hash_hex, expected);
        assert_eq!(report.hash_hex.len(), 64);

        value.operating_system.hostname = "another-pc".into();
        let changed = InventoryReport::from_inventory(value).unwrap();
        assert_ne!(report.hash_hex, changed.hash_hex);
    }
}
