//! Windows 设备库存的只读观测。

use anchor_protocol::HardwareInventory;
#[cfg(windows)]
use std::collections::HashMap;

#[cfg(windows)]
use anchor_protocol::{
    ComputerInventory, CpuInventory, DiskInventory, GpuInventory, MemoryModuleInventory,
    NetworkAdapterInventory, OperatingSystemInventory, OpticalDriveInventory, UsbDeviceInventory,
    VolumeInventory, HARDWARE_INVENTORY_SCHEMA_VERSION,
};
#[cfg(windows)]
use chrono::Utc;
#[cfg(windows)]
use serde::Deserialize;
#[cfg(windows)]
use wmi::{COMLibrary, WMIConnection};

use crate::{
    burning::BurnDriveInfo,
    error::PlatformError,
    nic::NicInfo,
    usb::{parse_usb_vendor_product, usb_storage_serial, UsbStorageDevice},
};

/// 一次设备库存观测的只读快照。
#[derive(Debug, Clone)]
pub struct DeviceInventory {
    pub usb_storage: Vec<UsbStorageDevice>,
    pub network_adapters: Vec<NicInfo>,
    pub burn_drives: Vec<BurnDriveInfo>,
}

/// 读取当前 Windows 设备库存；只查询在场设备，不执行任何设备状态修改。
pub fn observe_inventory() -> Result<DeviceInventory, PlatformError> {
    #[cfg(windows)]
    {
        use crate::{pnp, usb::is_usb_mass_storage_instance};

        let usb_storage = pnp::enumerate(&pnp::DISK_DRIVE_CLASS)?
            .into_iter()
            .filter(|device| is_usb_mass_storage_instance(&device.instance_id))
            .map(|device| {
                let (vendor_id, product_id) = device
                    .parent_instance_id()
                    .ok()
                    .flatten()
                    .map(|parent| parse_usb_vendor_product(&parent))
                    .unwrap_or((None, None));
                UsbStorageDevice {
                    serial: usb_storage_serial(&device.instance_id),
                    enabled: !device.disabled,
                    removable: device.removable,
                    instance_id: device.instance_id,
                    vendor_id,
                    product_id,
                }
            })
            .collect();

        let network_adapters = pnp::enumerate(&pnp::NET_CLASS)?
            .into_iter()
            .map(|device| NicInfo {
                name: device
                    .friendly_name
                    .unwrap_or_else(|| device.instance_id.clone()),
                adapter_id: device.instance_id,
                enabled: !device.disabled,
            })
            .collect();

        let burn_drives = pnp::enumerate(&pnp::CDROM_CLASS)?
            .into_iter()
            .map(|device| BurnDriveInfo {
                name: device
                    .friendly_name
                    .unwrap_or_else(|| device.instance_id.clone()),
                drive_id: device.instance_id,
                enabled: !device.disabled,
            })
            .collect();

        Ok(DeviceInventory {
            usb_storage,
            network_adapters,
            burn_drives,
        })
    }

    #[cfg(not(windows))]
    {
        Err(PlatformError::Unsupported("Windows 设备库存观测"))
    }
}

/// 采集注册和硬件变化上报使用的全量硬件快照。
///
/// 该入口独立于 `observe_inventory`：策略执行仍使用 CfgMgr32 的轻量库存，
/// 全量快照只在注册、心跳发现变化或人工刷新时调用。
pub fn collect_hardware_inventory() -> Result<HardwareInventory, PlatformError> {
    #[cfg(windows)]
    {
        collect_windows_hardware_inventory()
    }

    #[cfg(not(windows))]
    {
        Err(PlatformError::Unsupported("Windows 全量硬件清单采集"))
    }
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiComputerSystem {
    manufacturer: Option<String>,
    model: Option<String>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiBios {
    manufacturer: Option<String>,
    serial_number: Option<String>,
    #[serde(rename = "SMBIOSBIOSVersion")]
    smbios_bios_version: Option<String>,
    release_date: Option<String>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiBaseBoard {
    serial_number: Option<String>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiOperatingSystem {
    caption: Option<String>,
    version: Option<String>,
    build_number: Option<String>,
    #[serde(rename = "OSArchitecture")]
    os_architecture: Option<String>,
    locale: Option<String>,
    #[serde(rename = "CSName")]
    cs_name: Option<String>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiProcessor {
    manufacturer: Option<String>,
    name: Option<String>,
    number_of_cores: Option<u32>,
    number_of_logical_processors: Option<u32>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiPhysicalMemory {
    manufacturer: Option<String>,
    part_number: Option<String>,
    serial_number: Option<String>,
    capacity: Option<u64>,
    speed: Option<u32>,
    device_locator: Option<String>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiDiskDrive {
    model: Option<String>,
    serial_number: Option<String>,
    media_type: Option<String>,
    size: Option<u64>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiLogicalDisk {
    #[serde(rename = "DeviceID")]
    device_id: Option<String>,
    volume_name: Option<String>,
    file_system: Option<String>,
    size: Option<u64>,
    free_space: Option<u64>,
    volume_serial_number: Option<String>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiNetworkAdapter {
    #[serde(rename = "GUID")]
    guid: Option<String>,
    #[serde(rename = "NetConnectionID")]
    net_connection_id: Option<String>,
    name: Option<String>,
    #[serde(rename = "MACAddress")]
    mac_address: Option<String>,
    #[serde(rename = "NetConnectionStatus")]
    net_connection_status: Option<u16>,
    #[serde(rename = "PNPDeviceID")]
    pnp_device_id: Option<String>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiSignedDriver {
    #[serde(rename = "DeviceID")]
    device_id: Option<String>,
    driver_version: Option<String>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiPnpEntity {
    #[serde(rename = "DeviceID")]
    device_id: Option<String>,
    name: Option<String>,
    manufacturer: Option<String>,
    #[serde(rename = "PNPClass")]
    pnp_class: Option<String>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiOpticalDrive {
    #[serde(rename = "DeviceID")]
    device_id: Option<String>,
    name: Option<String>,
    media_loaded: Option<bool>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiVideoController {
    name: Option<String>,
    adapter_compatibility: Option<String>,
    #[serde(rename = "AdapterRAM")]
    adapter_ram: Option<u64>,
    driver_version: Option<String>,
}

#[cfg(windows)]
fn collect_windows_hardware_inventory() -> Result<HardwareInventory, PlatformError> {
    let com = COMLibrary::new()
        .map_err(|error| PlatformError::Failed(format!("初始化 COM 失败: {error}")))?;
    let wmi = WMIConnection::new(com)
        .map_err(|error| PlatformError::Failed(format!("初始化 WMI 失败: {error}")))?;

    let computer_system = wmi_query::<WmiComputerSystem>(
        &wmi,
        "SELECT Manufacturer, Model FROM Win32_ComputerSystem",
        "读取计算机信息",
    )?
    .into_iter()
    .next()
    .unwrap_or(WmiComputerSystem {
        manufacturer: None,
        model: None,
    });
    let bios = wmi_query::<WmiBios>(
        &wmi,
        "SELECT Manufacturer, SerialNumber, SMBIOSBIOSVersion, ReleaseDate FROM Win32_BIOS",
        "读取 BIOS 信息",
    )?
    .into_iter()
    .next()
    .unwrap_or(WmiBios {
        manufacturer: None,
        serial_number: None,
        smbios_bios_version: None,
        release_date: None,
    });
    let baseboard = wmi_query::<WmiBaseBoard>(
        &wmi,
        "SELECT SerialNumber FROM Win32_BaseBoard",
        "读取主板信息",
    )?
    .into_iter()
    .next()
    .unwrap_or(WmiBaseBoard {
        serial_number: None,
    });
    let operating_system = wmi_query::<WmiOperatingSystem>(
        &wmi,
        "SELECT Caption, Version, BuildNumber, OSArchitecture, Locale, CSName \
         FROM Win32_OperatingSystem",
        "读取操作系统信息",
    )?
    .into_iter()
    .next()
    .unwrap_or(WmiOperatingSystem {
        caption: None,
        version: None,
        build_number: None,
        os_architecture: None,
        locale: None,
        cs_name: None,
    });
    let processors = wmi_query::<WmiProcessor>(
        &wmi,
        "SELECT Manufacturer, Name, NumberOfCores, NumberOfLogicalProcessors \
         FROM Win32_Processor",
        "读取 CPU 信息",
    )?;
    let memory_modules = wmi_query::<WmiPhysicalMemory>(
        &wmi,
        "SELECT Manufacturer, PartNumber, SerialNumber, Capacity, Speed, DeviceLocator \
         FROM Win32_PhysicalMemory",
        "读取内存信息",
    )?;
    let disks = wmi_query::<WmiDiskDrive>(
        &wmi,
        "SELECT Model, SerialNumber, MediaType, Size FROM Win32_DiskDrive",
        "读取磁盘信息",
    )?;
    let volumes = wmi_query::<WmiLogicalDisk>(
        &wmi,
        "SELECT DeviceID, VolumeName, FileSystem, Size, FreeSpace, VolumeSerialNumber \
         FROM Win32_LogicalDisk",
        "读取卷信息",
    )?;
    let network_adapters = wmi_query::<WmiNetworkAdapter>(
        &wmi,
        "SELECT GUID, NetConnectionID, Name, MACAddress, NetConnectionStatus, PNPDeviceID \
         FROM Win32_NetworkAdapter WHERE NetConnectionStatus IS NOT NULL",
        "读取网卡信息",
    )?;
    let signed_drivers = wmi_query::<WmiSignedDriver>(
        &wmi,
        "SELECT DeviceID, DriverVersion FROM Win32_PnPSignedDriver",
        "读取驱动版本",
    )?;
    let usb_devices = wmi_query::<WmiPnpEntity>(
        &wmi,
        "SELECT DeviceID, Name, Manufacturer, PNPClass FROM Win32_PnPEntity \
         WHERE PNPDeviceID LIKE 'USB%'",
        "读取 USB 设备",
    )?;
    let optical_drives = wmi_query::<WmiOpticalDrive>(
        &wmi,
        "SELECT DeviceID, Name, MediaLoaded FROM Win32_CDROMDrive",
        "读取光驱信息",
    )?;
    let gpus = wmi_query::<WmiVideoController>(
        &wmi,
        "SELECT Name, AdapterCompatibility, AdapterRAM, DriverVersion \
         FROM Win32_VideoController",
        "读取显卡信息",
    )?;

    let driver_versions: HashMap<String, String> = signed_drivers
        .into_iter()
        .filter_map(|driver| {
            let device_id = text(driver.device_id).to_ascii_lowercase();
            let version = text(driver.driver_version);
            (!device_id.is_empty() && !version.is_empty()).then_some((device_id, version))
        })
        .collect();
    let processor_cores = processors
        .iter()
        .map(|processor| processor.number_of_cores.unwrap_or(0))
        .fold(0_u32, u32::saturating_add);
    let processor_threads = processors
        .iter()
        .map(|processor| processor.number_of_logical_processors.unwrap_or(0))
        .fold(0_u32, u32::saturating_add);
    let (cpu_manufacturer, cpu_model) = processors
        .first()
        .map(|processor| {
            (
                text(processor.manufacturer.clone()),
                text(processor.name.clone()),
            )
        })
        .unwrap_or_default();
    let serial_number = {
        let bios_serial = text(bios.serial_number);
        if bios_serial.is_empty() {
            text(baseboard.serial_number)
        } else {
            bios_serial
        }
    };

    Ok(HardwareInventory {
        schema_version: HARDWARE_INVENTORY_SCHEMA_VERSION,
        captured_at: Utc::now(),
        computer: ComputerInventory {
            manufacturer: text(computer_system.manufacturer),
            model: text(computer_system.model),
            serial_number,
            bios_vendor: text(bios.manufacturer),
            bios_version: text(bios.smbios_bios_version),
            bios_release_date: normalize_wmi_date(bios.release_date),
        },
        operating_system: OperatingSystemInventory {
            name: text(operating_system.caption),
            version: text(operating_system.version),
            build: text(operating_system.build_number),
            architecture: text(operating_system.os_architecture),
            locale: text(operating_system.locale),
            hostname: {
                let hostname = text(operating_system.cs_name);
                if hostname.is_empty() {
                    std::env::var("COMPUTERNAME").unwrap_or_default()
                } else {
                    hostname
                }
            },
        },
        cpu: CpuInventory {
            manufacturer: cpu_manufacturer,
            model: cpu_model,
            cores: processor_cores,
            threads: processor_threads,
        },
        memory_modules: memory_modules
            .into_iter()
            .map(|module| MemoryModuleInventory {
                manufacturer: text(module.manufacturer),
                part_number: text(module.part_number),
                serial_number: text(module.serial_number),
                capacity_bytes: module.capacity.unwrap_or(0),
                speed_mhz: module.speed.unwrap_or(0),
                slot: text(module.device_locator),
            })
            .collect(),
        disks: disks
            .into_iter()
            .map(|disk| DiskInventory {
                model: text(disk.model),
                serial_number: text(disk.serial_number),
                media_type: text(disk.media_type),
                size_bytes: disk.size.unwrap_or(0),
            })
            .collect(),
        volumes: volumes
            .into_iter()
            .map(|volume| VolumeInventory {
                name: text(volume.device_id),
                label: text(volume.volume_name),
                filesystem: text(volume.file_system),
                size_bytes: volume.size.unwrap_or(0),
                free_bytes: volume.free_space.unwrap_or(0),
                serial_number: text(volume.volume_serial_number),
            })
            .collect(),
        network_adapters: network_adapters
            .into_iter()
            .map(|adapter| {
                let pnp_device_id = text(adapter.pnp_device_id);
                let guid = text(adapter.guid);
                let name = {
                    let connection_name = text(adapter.net_connection_id);
                    if connection_name.is_empty() {
                        text(adapter.name)
                    } else {
                        connection_name
                    }
                };
                let instance_id = if pnp_device_id.is_empty() {
                    if guid.is_empty() {
                        name.clone()
                    } else {
                        guid
                    }
                } else {
                    pnp_device_id.clone()
                };
                NetworkAdapterInventory {
                    instance_id,
                    name,
                    mac_address: text(adapter.mac_address),
                    link_up: adapter.net_connection_status == Some(2),
                    driver_version: driver_versions
                        .get(&pnp_device_id.to_ascii_lowercase())
                        .cloned()
                        .unwrap_or_default(),
                }
            })
            .collect(),
        usb_devices: usb_devices
            .into_iter()
            .map(|device| {
                let device_id = text(device.device_id);
                let (vendor_id, product_id) = parse_usb_vendor_product(&device_id);
                UsbDeviceInventory {
                    serial_number: usb_instance_serial(&device_id),
                    instance_id: device_id,
                    name: text(device.name),
                    manufacturer: text(device.manufacturer),
                    vendor_id: vendor_id.unwrap_or_default(),
                    product_id: product_id.unwrap_or_default(),
                    device_class: text(device.pnp_class),
                }
            })
            .collect(),
        optical_drives: optical_drives
            .into_iter()
            .map(|drive| OpticalDriveInventory {
                instance_id: text(drive.device_id),
                name: text(drive.name),
                media_present: drive.media_loaded.unwrap_or(false),
            })
            .collect(),
        gpus: gpus
            .into_iter()
            .map(|gpu| GpuInventory {
                name: text(gpu.name),
                manufacturer: text(gpu.adapter_compatibility),
                memory_bytes: gpu.adapter_ram.unwrap_or(0),
                driver_version: text(gpu.driver_version),
            })
            .collect(),
    })
}

#[cfg(windows)]
fn wmi_query<T>(
    connection: &WMIConnection,
    query: &str,
    label: &str,
) -> Result<Vec<T>, PlatformError>
where
    T: serde::de::DeserializeOwned,
{
    connection
        .raw_query(query)
        .map_err(|error| PlatformError::Failed(format!("{label}失败: {error}")))
}

#[cfg(windows)]
fn text(value: Option<String>) -> String {
    value.unwrap_or_default().trim().to_owned()
}

#[cfg(windows)]
fn normalize_wmi_date(value: Option<String>) -> String {
    let raw = text(value);
    if raw.len() >= 8 && raw.as_bytes()[..8].iter().all(u8::is_ascii_digit) {
        format!("{}-{}-{}", &raw[0..4], &raw[4..6], &raw[6..8])
    } else {
        raw
    }
}

#[cfg(windows)]
fn usb_instance_serial(instance_id: &str) -> String {
    instance_id
        .rsplit('\\')
        .next()
        .filter(|value| !value.is_empty() && !value.contains('&'))
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn wmi_dates_are_reduced_to_iso_dates() {
        assert_eq!(
            normalize_wmi_date(Some("20240102000000.000000+000".into())),
            "2024-01-02"
        );
        assert_eq!(normalize_wmi_date(Some("2024-01-02".into())), "2024-01-02");
        assert_eq!(normalize_wmi_date(Some("unknown".into())), "unknown");
    }

    #[cfg(windows)]
    #[test]
    fn usb_serial_is_only_used_when_the_instance_tail_looks_stable() {
        assert_eq!(
            usb_instance_serial(r"USB\VID_1234&PID_5678\ABC123"),
            "ABC123"
        );
        assert!(usb_instance_serial(r"USB\VID_1234&PID_5678\5&1&0&2").is_empty());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "读取本机 WMI 全量硬件清单"]
    fn full_hardware_inventory_can_be_collected_from_the_local_machine() {
        let inventory = collect_hardware_inventory().unwrap();

        assert_eq!(
            inventory.schema_version,
            anchor_protocol::HARDWARE_INVENTORY_SCHEMA_VERSION
        );
        assert!(!inventory.operating_system.hostname.trim().is_empty());
        assert!(!inventory.cpu.model.trim().is_empty());
        assert!(!inventory.computer.model.trim().is_empty());
        assert!(!inventory.volumes.is_empty());
        assert!(!inventory.network_adapters.is_empty());
    }

    /// 真实设备观测：只读查询本机，确认每类设备都带有可用于策略匹配的标识。
    #[cfg(windows)]
    #[test]
    fn inventory_observation_carries_stable_device_ids() {
        let inventory = observe_inventory().unwrap();

        assert!(inventory
            .network_adapters
            .iter()
            .all(|adapter| !adapter.adapter_id.trim().is_empty()));
        assert!(inventory
            .usb_storage
            .iter()
            .all(|device| !device.instance_id.trim().is_empty()));
        assert!(inventory
            .burn_drives
            .iter()
            .all(|drive| !drive.drive_id.trim().is_empty()));
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_inventory_is_explicitly_unsupported() {
        assert!(matches!(
            observe_inventory(),
            Err(PlatformError::Unsupported("Windows 设备库存观测"))
        ));
    }
}
