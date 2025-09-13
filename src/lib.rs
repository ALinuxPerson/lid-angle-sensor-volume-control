use anyhow::Context;
use objc2_core_foundation::{CFAllocator, CFDictionary, CFIndex, CFNumber, CFRetained, CFString};
use objc2_io_kit::{
    IOHIDDevice, IOHIDManager, IOHIDReportType, kIOHIDOptionsTypeNone, kIOReturnSuccess,
};
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ptr;
use std::ptr::NonNull;

const VENDOR_ID: u16 = 0x05AC;
const PRODUCT_ID: u16 = 0x8104;
const USAGE_PAGE: u16 = 0x0020;
const USAGE: u16 = 0x008A;

fn matching() -> CFRetained<CFDictionary<CFString, CFNumber>> {
    let vendor_id_str = CFString::from_static_str("VendorID");
    let product_id_str = CFString::from_static_str("ProductID");
    let usage_page_str = CFString::from_static_str("UsagePage");
    let usage_str = CFString::from_static_str("Usage");
    let vendor_id_num = CFNumber::new_i16(VENDOR_ID as _);
    let product_id_num = CFNumber::new_i16(PRODUCT_ID as _);
    let usage_page_num = CFNumber::new_i16(USAGE_PAGE as _);
    let usage_num = CFNumber::new_i16(USAGE as _);
    CFDictionary::from_slices(
        &[
            vendor_id_str.deref(),
            &product_id_str,
            &usage_page_str,
            &usage_str,
        ],
        &[
            vendor_id_num.deref(),
            &product_id_num,
            &usage_page_num,
            &usage_num,
        ],
    )
}

fn find_lid_angle_sensor() -> anyhow::Result<Option<CFRetained<IOHIDDevice>>> {
    let allocator = CFAllocator::default().context("failed to initialize CFAllocator")?;
    let manager = unsafe { IOHIDManager::new(Some(&allocator), kIOHIDOptionsTypeNone) };
    unsafe { manager.open(kIOHIDOptionsTypeNone) };
    unsafe { manager.set_device_matching(Some(matching().as_opaque())) };

    let devices =
        unsafe { manager.devices() }.context("failed to obtain devices from IOHIDManager")?;
    let devices_count = devices.count();
    if devices_count <= 0 {
        return Ok(None);
    }

    let mut values_c_void = ManuallyDrop::new(vec![ptr::null(); devices_count as _]);
    unsafe { devices.values(values_c_void.as_mut_ptr()) };

    let devices = unsafe {
        Vec::from_raw_parts(
            values_c_void.as_mut_ptr() as *mut *const IOHIDDevice,
            values_c_void.len(),
            values_c_void.capacity(),
        )
    };

    for device in devices {
        if unsafe { (*device).open(kIOHIDOptionsTypeNone) } != kIOReturnSuccess {
            continue;
        }

        let mut test_report = [0u8; 8];
        let mut report_len = test_report.len() as CFIndex;
        if unsafe {
            (*device).report(
                IOHIDReportType::Feature,
                1,
                NonNull::new_unchecked(test_report.as_mut_ptr()),
                NonNull::from_mut(&mut report_len),
            )
        } != kIOReturnSuccess
        {
            continue;
        }

        if report_len >= 3 {
            return Ok(Some(unsafe {
                CFRetained::retain(NonNull::new_unchecked(device as *mut IOHIDDevice))
            }));
        }
    }

    Ok(None)
}

pub struct LidAngleSensor {
    hid_device: CFRetained<IOHIDDevice>,
}

impl LidAngleSensor {
    pub fn new() -> anyhow::Result<Option<Self>> {
        let hid_device = match find_lid_angle_sensor()? {
            Some(hid_device) => hid_device,
            None => return Ok(None),
        };

        let ret = unsafe { hid_device.open(kIOHIDOptionsTypeNone) };
        anyhow::ensure!(
            ret == kIOReturnSuccess,
            "failed to open lid angle sensor hid device with IOReturn {ret}"
        );

        Ok(Some(Self { hid_device }))
    }

    pub fn lid_angle(&self) -> anyhow::Result<u16> {
        let mut report = [0; 8];
        let mut report_len = report.len() as CFIndex;

        let ret = unsafe {
            self.hid_device.report(
                IOHIDReportType::Feature,
                1,
                NonNull::new_unchecked(report.as_mut_ptr()),
                NonNull::from_mut(&mut report_len),
            )
        };
        anyhow::ensure!(
            ret == kIOReturnSuccess,
            "failed to get lid angle from lid angle sensor hid device with IOReturn {ret}",
        );
        anyhow::ensure!(report_len >= 3, "lid angle report from sensor hid device has invalid input, expected report len >= 3, got {report_len}");
        Ok(u16::from_le_bytes([report[1], report[2]]))
    }
}
