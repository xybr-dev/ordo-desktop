//! Windows monitor identity for display recognition (T1.6d, Decision 312).
//!
//! GPUI's Windows display uuid is derived from the GDI device name
//! (`\\.\DISPLAYn`), so it names a port, not a monitor. This module reads the
//! monitor's friendly name and EDID maker/product codes through the display
//! configuration API and keys them by that same uuid, so the join to GPUI's
//! displays is exact.
//!
//! This is the workspace's second audited `unsafe` module, beside
//! `process_memory` (Decision 312). `cargo xtask verify` allows exactly these
//! two. Everything outside the Windows inner module is safe and pure.

use std::collections::HashMap;

use uuid::Uuid;

/// Monitor names keyed by GPUI display uuid string, read now. Empty off
/// Windows and on any failure; a display with no usable name is absent.
pub fn read() -> HashMap<String, String> {
    imp::read()
}

/// The uuid GPUI derives for a display from its fixed-size GDI device name
/// (`MONITORINFOEXW::szDevice`, trailing NULs included): a v5 uuid in the DNS
/// namespace over the UTF-16 units flattened big-endian.
pub fn gpui_uuid(gdi_device_name: &[u16; 32]) -> String {
    let bytes: Vec<u8> = gdi_device_name
        .iter()
        .flat_map(|unit| unit.to_be_bytes())
        .collect();
    Uuid::new_v5(&Uuid::NAMESPACE_DNS, &bytes).to_string()
}

/// Decodes the three-letter PNP maker id from the value Windows reports.
///
/// EDID stores the maker as a big-endian 16-bit value (one zero bit, then
/// three 5-bit letters, `A` = 1); the display configuration API hands it back
/// byte-swapped, so Dell's EDID `0x10AC` arrives as `0xAC10`.
pub fn pnp_maker(edid_manufacture_id: u16) -> Option<String> {
    let value = edid_manufacture_id.swap_bytes();
    [(value >> 10) & 0x1F, (value >> 5) & 0x1F, value & 0x1F]
        .into_iter()
        .map(|letter| match letter {
            1..=26 => u8::try_from(letter + u16::from(b'A') - 1)
                .ok()
                .map(char::from),
            _ => None,
        })
        .collect()
}

/// The recognition name for a monitor: its friendly name with the EDID maker
/// and product, e.g. `EPSON PJ (EPS/0x0123)`. Blank or generic friendly names
/// count as no name. Without valid EDID ids the friendly name stands alone.
pub fn normalize_name(
    friendly: &str,
    edid_manufacture_id: u16,
    edid_product_code: u16,
    edid_ids_valid: bool,
) -> Option<String> {
    let friendly = friendly.trim();
    let generic = ["generic pnp monitor", "generic non-pnp monitor"];
    if friendly.is_empty() || generic.iter().any(|g| friendly.eq_ignore_ascii_case(g)) {
        return None;
    }
    if !edid_ids_valid {
        return Some(friendly.to_owned());
    }
    match pnp_maker(edid_manufacture_id) {
        Some(maker) => Some(format!("{friendly} ({maker}/0x{edid_product_code:04X})")),
        None => Some(friendly.to_owned()),
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use std::collections::HashMap;
    use std::mem::size_of;

    use windows_sys::Win32::Devices::Display::{
        DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
        DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
        DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes,
        QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
    };
    use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};

    /// `edidIdsValid` is bit 2 of `DISPLAYCONFIG_TARGET_DEVICE_NAME_FLAGS`.
    const EDID_IDS_VALID: u32 = 1 << 2;

    pub(super) fn read() -> HashMap<String, String> {
        let mut names = HashMap::new();
        let Some(paths) = active_paths() else {
            return names;
        };
        for path in &paths {
            if let Some((uuid, name)) = identity(path) {
                names.insert(uuid, name);
            }
        }
        names
    }

    fn active_paths() -> Option<Vec<DISPLAYCONFIG_PATH_INFO>> {
        // The topology can change between sizing and querying, which the API
        // reports as insufficient buffer; one retry covers it.
        for _ in 0..2 {
            let mut path_count = 0u32;
            let mut mode_count = 0u32;
            // SAFETY: both out-pointers refer to live, initialized `u32`s
            // that the call writes once; no pointer is retained.
            let status = unsafe {
                GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
            };
            if status != ERROR_SUCCESS {
                return None;
            }
            let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
            // SAFETY: `paths` and `modes` are live vectors with exactly
            // `path_count` and `mode_count` initialized elements, the sizes
            // the OS just reported and passed back as the capacities. The
            // call writes at most that many elements and updates the counts;
            // the topology-id pointer may be null for active-path queries.
            let status = unsafe {
                QueryDisplayConfig(
                    QDC_ONLY_ACTIVE_PATHS,
                    &mut path_count,
                    paths.as_mut_ptr(),
                    &mut mode_count,
                    modes.as_mut_ptr(),
                    std::ptr::null_mut(),
                )
            };
            if status == ERROR_INSUFFICIENT_BUFFER {
                continue;
            }
            if status != ERROR_SUCCESS {
                return None;
            }
            paths.truncate(path_count as usize);
            return Some(paths);
        }
        None
    }

    fn identity(path: &DISPLAYCONFIG_PATH_INFO) -> Option<(String, String)> {
        let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
        source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
        source.header.size = u32::try_from(size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>()).ok()?;
        source.header.adapterId = path.sourceInfo.adapterId;
        source.header.id = path.sourceInfo.id;
        // SAFETY: `source` is a live `DISPLAYCONFIG_SOURCE_DEVICE_NAME` whose
        // header `type` and `size` Ordo set to match the struct, so the OS
        // writes no more than `size` bytes through the header pointer.
        let status = unsafe { DisplayConfigGetDeviceInfo(&mut source.header) };
        if status != 0 {
            return None;
        }

        let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
        target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
        target.header.size = u32::try_from(size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>()).ok()?;
        target.header.adapterId = path.targetInfo.adapterId;
        target.header.id = path.targetInfo.id;
        // SAFETY: as above, for a live `DISPLAYCONFIG_TARGET_DEVICE_NAME`
        // whose header `type` and `size` match the struct.
        let status = unsafe { DisplayConfigGetDeviceInfo(&mut target.header) };
        if status != 0 {
            return None;
        }

        let friendly = &target.monitorFriendlyDeviceName;
        let end = friendly
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(friendly.len());
        let friendly = String::from_utf16_lossy(&friendly[..end]);
        // SAFETY: the union was zero-initialized and then written by the OS;
        // every bit pattern is a valid `u32`, so reading `value` is sound.
        let flags = unsafe { target.flags.Anonymous.value };
        let name = super::normalize_name(
            &friendly,
            target.edidManufactureId,
            target.edidProductCodeId,
            flags & EDID_IDS_VALID != 0,
        )?;
        Some((super::gpui_uuid(&source.viewGdiDeviceName), name))
    }
}

#[cfg(not(windows))]
mod imp {
    use std::collections::HashMap;

    pub(super) fn read() -> HashMap<String, String> {
        HashMap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str) -> [u16; 32] {
        let mut buffer = [0u16; 32];
        for (slot, unit) in buffer.iter_mut().zip(name.encode_utf16()) {
            *slot = unit;
        }
        buffer
    }

    #[test]
    fn gpui_uuid_hashes_the_whole_nul_padded_array_big_endian() {
        let bytes: Vec<u8> = device(r"\\.\DISPLAY1")
            .iter()
            .flat_map(|unit| unit.to_be_bytes())
            .collect();
        assert_eq!(bytes.len(), 64);
        assert_eq!(
            gpui_uuid(&device(r"\\.\DISPLAY1")),
            Uuid::new_v5(&Uuid::NAMESPACE_DNS, &bytes).to_string()
        );
        assert_ne!(
            gpui_uuid(&device(r"\\.\DISPLAY1")),
            gpui_uuid(&device(r"\\.\DISPLAY2"))
        );
        let text = gpui_uuid(&device(r"\\.\DISPLAY1"));
        assert_eq!(text.len(), 36);
        assert_eq!(text.as_bytes()[14], b'5');
        assert_eq!(text, text.to_lowercase());
    }

    #[test]
    fn gpui_uuid_is_pinned_to_a_known_value() {
        // Pinned so a change to the derivation, and so a mismatch with GPUI's
        // own uuid, is noticed.
        assert_eq!(
            gpui_uuid(&device(r"\\.\DISPLAY1")),
            "78692035-3a04-52f3-a2bf-1133155ae828"
        );
    }

    #[test]
    fn maker_decodes_from_the_byte_swapped_value() {
        assert_eq!(pnp_maker(0xAC10).as_deref(), Some("DEL"));
        assert_eq!(pnp_maker(0x2D4C).as_deref(), Some("SAM"));
        assert_eq!(pnp_maker(0x0000), None);
        assert_eq!(pnp_maker(0xFFFF), None);
    }

    #[test]
    fn friendly_name_carries_maker_and_product() {
        assert_eq!(
            normalize_name("  DELL U2415 ", 0xAC10, 0x41B5, true).as_deref(),
            Some("DELL U2415 (DEL/0x41B5)")
        );
        assert_eq!(
            normalize_name("EPSON PJ", 0x2D4C, 0x0123, true).as_deref(),
            Some("EPSON PJ (SAM/0x0123)")
        );
    }

    #[test]
    fn invalid_edid_ids_leave_the_friendly_name_alone() {
        assert_eq!(
            normalize_name("Projector", 0xAC10, 0x1, false).as_deref(),
            Some("Projector")
        );
        assert_eq!(
            normalize_name("Projector", 0, 0, true).as_deref(),
            Some("Projector")
        );
    }

    #[test]
    fn blank_and_generic_names_count_as_no_name() {
        for friendly in [
            "",
            "   ",
            "Generic PnP Monitor",
            "generic non-pnp monitor",
            " GENERIC PNP MONITOR ",
        ] {
            assert_eq!(
                normalize_name(friendly, 0xAC10, 1, true),
                None,
                "{friendly:?}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn reading_the_display_configuration_does_not_panic() {
        let names = read();
        assert!(names.values().all(|name| !name.trim().is_empty()));
    }
}
