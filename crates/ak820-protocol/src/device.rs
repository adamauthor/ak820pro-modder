use hidapi::{HidApi, HidDevice};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, trace};

use crate::commands::keymap::{Keymap, KEYMAP_BYTES};
use crate::commands::lighting::{self, LightingConfig};
use crate::commands::macros::{
    self as macro_cmds, IndexEntry, Macro, MACRO_DATA_ADDR, MACRO_INDEX_BYTES,
};
use crate::commands::per_key_rgb::{CustomLedMap, CUSTOM_LED_BYTES};
use crate::commands::system::{DeviceInfoReport, GameMode};
use crate::commands::tft::{build_tft_header, TftAnimation};
use crate::protocol::{
    build_frame, cmd, HEADER_LEN, MAGIC_INCOMING, PACKET_LEN, PAYLOAD_PER_PACKET, REPORT_ID,
};
use crate::{error::*, CONTROL_INTERFACE, PRODUCT_IDS, VENDOR_ID};

/// Open vendor interfaces without seizing normal keyboard input on macOS.
fn hid_api() -> Result<HidApi> {
    let api = HidApi::new()?;
    #[cfg(target_os = "macos")]
    api.set_open_exclusive(false);
    Ok(api)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceProtocol {
    WebOutput,
    LegacyFeature,
}

const CONTROL_PAGES: &[u16] = &[0xFF68, 0xFF80, 0xFF60, 0xFF00, 0xFF01, 0xFF1B];

/// Default response timeout (ms) for non-streaming GET/SET commands.
const DEFAULT_TIMEOUT_MS: i32 = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub protocol: DeviceProtocol,
    pub is_control: bool,
    pub is_tft: bool,
    pub vid: u16,
    pub pid: u16,
    pub interface: i32,
    pub usage_page: u16,
    pub usage: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub path: String,
}

/// Summary of one HID interface's report-descriptor properties.
///
/// We use this to find the right interface for chunked TFT uploads: the
/// `SET_TFT_USER_ANIMATION` path needs an output-report payload of ~4096
/// bytes, which is wildly different from the 64-byte control interface.
/// Opening the wrong interface and writing big chunks ends with cryptic
/// "frame too long" / IOHIDDevice errors on macOS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceProbe {
    /// Per-interface DeviceInfo (path, usage_page, product, …).
    pub info: DeviceInfo,
    /// Largest output-report size observed in the descriptor, in bytes
    /// (excluding the report-ID prefix). `None` means we couldn't open the
    /// device or the descriptor was empty.
    pub max_output_report_bytes: Option<usize>,
    /// Raw report descriptor bytes for the curious / for future RE.
    pub raw_descriptor_hex: Option<String>,
}

/// Open every AK820 candidate, read its report descriptor, and return a
/// summary used by RE / interface-selection logic.
///
/// macOS handles are non-exclusive. Only vendor endpoints are opened.
pub fn probe_interfaces() -> Result<Vec<InterfaceProbe>> {
    let api = hid_api()?;
    let infos = enumerate()?;
    let mut out = Vec::with_capacity(infos.len());
    for info in infos {
        if !(info.is_control || info.is_tft) {
            out.push(InterfaceProbe {
                info,
                max_output_report_bytes: None,
                raw_descriptor_hex: None,
            });
            continue;
        }
        let opened = api.open_path(&std::ffi::CString::new(info.path.clone()).unwrap());
        let mut probe = InterfaceProbe {
            info: info.clone(),
            max_output_report_bytes: None,
            raw_descriptor_hex: None,
        };
        if let Ok(dev) = opened {
            let mut buf = vec![0u8; 4096];
            if let Ok(n) = dev.get_report_descriptor(&mut buf) {
                let desc = &buf[..n];
                probe.raw_descriptor_hex = Some(hex::encode(desc));
                probe.max_output_report_bytes = parse_max_output_report_size(desc);
            }
        }
        out.push(probe);
    }
    Ok(out)
}

/// Minimal HID-descriptor walker: returns the largest payload size (in bytes)
/// of any Output (= 0x91) report defined in the descriptor.
///
/// HID descriptors are a stream of variable-length items. Each item starts
/// with a 1-byte tag that encodes (size, type, tag). We only care about the
/// **Global** items `Report Size` (0x75) and `Report Count` (0x95), plus the
/// **Main** item `Output` (0x91). Whenever an `Output` is emitted, the most
/// recent Report Size × Report Count gives that report's payload bit-width;
/// we round up to bytes and take the max across all such Output emissions.
///
/// Not a full parser — but good enough to identify the "big report"
/// interface used for TFT uploads on the AK820 Pro.
fn parse_max_output_report_size(desc: &[u8]) -> Option<usize> {
    let mut i = 0;
    let mut report_size_bits: u32 = 0;
    let mut report_count: u32 = 0;
    let mut max_bytes: usize = 0;
    while i < desc.len() {
        let head = desc[i];
        let size_code = head & 0x03;
        let size = match size_code {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 4,
            _ => 0,
        };
        let tag = head & 0xFC;
        if i + 1 + size > desc.len() {
            break;
        }
        let data: u32 = match size {
            0 => 0,
            1 => desc[i + 1] as u32,
            2 => (desc[i + 1] as u32) | ((desc[i + 2] as u32) << 8),
            4 => {
                (desc[i + 1] as u32)
                    | ((desc[i + 2] as u32) << 8)
                    | ((desc[i + 3] as u32) << 16)
                    | ((desc[i + 4] as u32) << 24)
            }
            _ => 0,
        };
        match tag {
            // Global: Report Size (bits per field)
            0x74 => report_size_bits = data,
            // Global: Report Count (number of fields)
            0x94 => report_count = data,
            // Main: Output
            0x90 => {
                let bits = report_size_bits.saturating_mul(report_count);
                let bytes = bits.div_ceil(8) as usize;
                if bytes > max_bytes {
                    max_bytes = bytes;
                }
            }
            _ => {}
        }
        i += 1 + size;
    }
    if max_bytes == 0 {
        None
    } else {
        Some(max_bytes)
    }
}

pub fn enumerate() -> Result<Vec<DeviceInfo>> {
    let api = hid_api()?;
    let mut found = Vec::new();
    for d in api.device_list() {
        if d.vendor_id() != VENDOR_ID || !PRODUCT_IDS.contains(&d.product_id()) {
            continue;
        }
        // Sonix IDs are shared by unrelated keyboards. Require the observed
        // AK820 product name for the newly supported PID.
        if d.product_id() == 0x800A
            && !d
                .product_string()
                .is_some_and(|p| p.to_ascii_uppercase().starts_with("AK820"))
        {
            continue;
        }
        let info = DeviceInfo {
            protocol: DeviceProtocol::WebOutput,
            is_control: false,
            is_tft: false,
            vid: d.vendor_id(),
            pid: d.product_id(),
            interface: d.interface_number(),
            usage_page: d.usage_page(),
            usage: d.usage(),
            manufacturer: d.manufacturer_string().map(str::to_owned),
            product: d.product_string().map(str::to_owned),
            serial: d.serial_number().map(str::to_owned),
            path: d.path().to_string_lossy().into_owned(),
        };
        debug!(?info, "candidate HID device");
        found.push(info);
    }
    classify_candidates(&mut found);
    debug!(count = found.len(), "enumerated AK820 candidates");
    Ok(found)
}

/// FF13 + FF68/4096 is the feature-report firmware, not the web-driver
/// firmware whose FF68 is control and FF67 is a 4104-byte TFT endpoint.
fn classify_candidates(found: &mut [DeviceInfo]) {
    let legacy: Vec<_> = found
        .iter()
        .filter(|d| d.usage_page == 0xFF13)
        .map(|d| (d.pid, d.product.clone()))
        .collect();
    let has_web_control = found.iter().any(|d| {
        !legacy.contains(&(d.pid, d.product.clone())) && CONTROL_PAGES.contains(&d.usage_page)
    });
    for d in found {
        d.protocol = if legacy.contains(&(d.pid, d.product.clone())) {
            DeviceProtocol::LegacyFeature
        } else {
            DeviceProtocol::WebOutput
        };
        d.is_control = match d.protocol {
            DeviceProtocol::LegacyFeature => d.usage_page == 0xFF13,
            DeviceProtocol::WebOutput => {
                CONTROL_PAGES.contains(&d.usage_page)
                    || (!has_web_control && d.interface == CONTROL_INTERFACE)
            }
        };
        d.is_tft = match d.protocol {
            DeviceProtocol::LegacyFeature => d.usage_page == 0xFF68,
            DeviceProtocol::WebOutput => d.usage_page == 0xFF67,
        };
    }
}

pub struct Connection {
    device: HidDevice,
    info: DeviceInfo,
}

impl Connection {
    /// Open the AK820 control endpoint. Selection precedence:
    ///   1. `AK820_IFACE=N` → first device with `interface_number == N`
    ///   2. `AK820_USAGE_PAGE=0xFFXX` → first device with that usage page
    ///   3. default: profile-matched vendor control interface
    pub fn open_control() -> Result<Self> {
        let candidates = enumerate()?;

        let by_iface_env = std::env::var("AK820_IFACE")
            .ok()
            .and_then(|v| v.parse::<i32>().ok());
        let by_usage_env = std::env::var("AK820_USAGE_PAGE")
            .ok()
            .and_then(|v| u16::from_str_radix(v.trim_start_matches("0x"), 16).ok());

        let eligible: Vec<_> = candidates.iter().filter(|d| d.is_control).collect();
        // Keep explicit diagnostic overrides for existing web-profile devices.
        // Never permit them to select the legacy TFT as a control endpoint.
        let override_eligible =
            |d: &&DeviceInfo| d.protocol == DeviceProtocol::WebOutput || d.is_control;
        let pick = if let Some(want) = by_iface_env {
            candidates
                .iter()
                .filter(override_eligible)
                .find(|d| d.interface == want)
                .cloned()
        } else if let Some(want) = by_usage_env {
            candidates
                .iter()
                .filter(override_eligible)
                .find(|d| d.usage_page == want)
                .cloned()
        } else {
            eligible.first().copied().cloned()
        };

        let control = pick.ok_or(Error::DeviceNotFound {
            vid: VENDOR_ID,
            interface: CONTROL_INTERFACE,
        })?;

        let api = hid_api()?;
        let device = api.open_path(&std::ffi::CString::new(control.path.clone()).unwrap())?;
        device.set_blocking_mode(true)?;
        info!(
            path = %control.path,
            interface = control.interface,
            usage_page = format!("0x{:04x}", control.usage_page),
            "opened control interface"
        );
        Ok(Self {
            device,
            info: control,
        })
    }

    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    pub fn raw(&self) -> &HidDevice {
        &self.device
    }

    pub fn probe(&self) -> Result<ProbeReport> {
        if self.info.protocol == DeviceProtocol::LegacyFeature {
            let _guard = crate::legacy::TransactionLock::acquire(self.info.pid)?;
            let mut feature = [0u8; 65];
            if self.device.get_feature_report(&mut feature)? != feature.len() {
                return Err(Error::UnexpectedResponse(
                    "Short legacy read-only feature report".into(),
                ));
            }
        }
        Ok(ProbeReport {
            connected: true,
            interface: self.info.interface,
            product: self.info.product.clone(),
            firmware_version: None,
        })
    }

    /// Send one output report on this interface.
    fn write_output_report(&self, frame: &[u8; PACKET_LEN]) -> Result<()> {
        if self.info.protocol == DeviceProtocol::LegacyFeature {
            return Err(Error::NotImplemented("This onboard operation is not decoded for feature-report firmware. Use the supported Lighting, System, TFT or Mac keyboard controls."));
        }
        let mut buf = [0u8; PACKET_LEN + 1];
        buf[0] = REPORT_ID;
        buf[1..].copy_from_slice(frame);
        trace!(report_id = REPORT_ID, hex = %hex::encode(&buf[1..16]), "TX");
        self.device.write(&buf)?;
        Ok(())
    }

    /// Read one input report until either it matches `expected_cmd` or the
    /// timeout elapses. Returns the payload (bytes 8…end of one frame).
    fn read_response(&self, expected_cmd: u8, timeout_ms: i32) -> Result<Vec<u8>> {
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms as u64);
        let mut buf = [0u8; PACKET_LEN];
        loop {
            let remaining_ms = deadline
                .saturating_duration_since(std::time::Instant::now())
                .as_millis()
                .min(i32::MAX as u128) as i32;
            if remaining_ms == 0 {
                return Err(Error::UnexpectedResponse(format!(
                    "timeout waiting for cmd 0x{:02x}",
                    expected_cmd,
                )));
            }
            let n = self.device.read_timeout(&mut buf, remaining_ms)?;
            if n == 0 {
                continue;
            }
            if buf[0] != MAGIC_INCOMING {
                trace!(magic = buf[0], "ignoring non-response packet");
                continue;
            }
            if buf[1] != expected_cmd {
                trace!(cmd = buf[1], "ignoring response for different cmd");
                continue;
            }
            return Ok(buf[HEADER_LEN..n.min(PACKET_LEN)].to_vec());
        }
    }

    /// Run a single-chunk GET transaction: send request header, await response,
    /// return up to `content_size` payload bytes from the first response packet.
    fn get(&self, cmd_byte: u8, content_size: usize) -> Result<Vec<u8>> {
        if content_size > PAYLOAD_PER_PACKET {
            // Multi-chunk responses are needed for things like custom-LED (512 bytes)
            // — implement when we reach Phase 4/5. For now we only need single packets.
            return Err(Error::NotImplemented("multi-chunk GET not yet supported"));
        }
        let frame = build_frame(cmd_byte, content_size as u8, 0, &[], true);
        trace!(cmd = cmd_byte, content_size, "GET");
        self.write_output_report(&frame)?;
        let payload = self.read_response(cmd_byte, DEFAULT_TIMEOUT_MS)?;
        Ok(payload.into_iter().take(content_size).collect())
    }

    /// Run a single-chunk SET transaction.
    fn set(&self, cmd_byte: u8, payload: &[u8]) -> Result<()> {
        if payload.len() > PAYLOAD_PER_PACKET {
            return Err(Error::NotImplemented("multi-chunk SET not yet supported"));
        }
        let frame = build_frame(cmd_byte, payload.len() as u8, 0, payload, true);
        trace!(cmd = cmd_byte, len = payload.len(), "SET");
        self.write_output_report(&frame)?;
        // Set commands echo a response — drain it so it doesn't pollute the
        // next read. Ignore errors: some firmwares don't respond to SET ops.
        let _ = self.read_response(cmd_byte, DEFAULT_TIMEOUT_MS);
        Ok(())
    }

    /// Multi-chunk GET transaction. Sends one request per chunk and concatenates
    /// the responses. Mirrors the official driver's `C()` loop for content sizes
    /// that exceed a single packet's 56-byte payload (e.g. keymap reads at 512 B).
    fn get_many(&self, cmd_byte: u8, content_size: usize) -> Result<Vec<u8>> {
        let chunk = PAYLOAD_PER_PACKET;
        let num_chunks = content_size.div_ceil(chunk).max(1);
        let mut out = Vec::with_capacity(content_size);
        for i in 0..num_chunks {
            let addr = (i * chunk) as u16;
            let remaining = content_size - i * chunk;
            let this_size = remaining.min(chunk);
            let is_last = i == num_chunks - 1;
            let frame = build_frame(cmd_byte, this_size as u8, addr, &[], is_last);
            trace!(cmd = cmd_byte, chunk = i, addr, this_size, "GET chunk");
            self.write_output_report(&frame)?;
            let payload = self.read_response(cmd_byte, DEFAULT_TIMEOUT_MS)?;
            let take = payload.len().min(this_size);
            out.extend_from_slice(&payload[..take]);
        }
        out.truncate(content_size);
        Ok(out)
    }

    /// Multi-chunk SET transaction. Slices the payload into ≤56-byte chunks
    /// and sends one request per chunk; the firmware acks each.
    fn set_many(&self, cmd_byte: u8, payload: &[u8]) -> Result<()> {
        self.set_many_at(cmd_byte, 0, payload, true)
    }

    /// Multi-chunk GET starting at an arbitrary base address.
    ///
    /// Matches the official driver's `C()` semantics: addr per chunk is
    /// `addr_base + i * PAYLOAD_PER_PACKET`. The `last_packet` flag stays
    /// `false` for non-final chunks of a GET (the firmware ignores it for
    /// reads, so this only matters for paired SET writes — see `set_many_at`).
    fn get_many_at(&self, cmd_byte: u8, addr_base: u16, content_size: usize) -> Result<Vec<u8>> {
        let chunk = PAYLOAD_PER_PACKET;
        let num_chunks = content_size.div_ceil(chunk).max(1);
        let mut out = Vec::with_capacity(content_size);
        for i in 0..num_chunks {
            let addr = addr_base.wrapping_add((i * chunk) as u16);
            let remaining = content_size - i * chunk;
            let this_size = remaining.min(chunk);
            let is_last = i == num_chunks - 1;
            let frame = build_frame(cmd_byte, this_size as u8, addr, &[], is_last);
            trace!(cmd = cmd_byte, chunk = i, addr, this_size, "GET chunk");
            self.write_output_report(&frame)?;
            let payload = self.read_response(cmd_byte, DEFAULT_TIMEOUT_MS)?;
            let take = payload.len().min(this_size);
            out.extend_from_slice(&payload[..take]);
        }
        out.truncate(content_size);
        Ok(out)
    }

    /// Multi-chunk SET starting at an arbitrary base address with explicit
    /// commit semantics.
    ///
    /// - `addr_base`: bytes 3..4 of each frame increment from this base.
    /// - `commit_on_last`: if `true`, the final chunk sets the last-packet
    ///   flag (byte 6 = 1), telling the firmware to commit the transaction.
    ///   If `false`, *every* chunk has the flag cleared (used for two-phase
    ///   writes like `SET_MACRO` where the index page is written first with
    ///   `commit=false` and the data area follows with `commit=true`).
    fn set_many_at(
        &self,
        cmd_byte: u8,
        addr_base: u16,
        payload: &[u8],
        commit_on_last: bool,
    ) -> Result<()> {
        let chunk = PAYLOAD_PER_PACKET;
        let num_chunks = payload.len().div_ceil(chunk).max(1);
        for i in 0..num_chunks {
            let addr = addr_base.wrapping_add((i * chunk) as u16);
            let start = i * chunk;
            let this_size = (payload.len() - start).min(chunk);
            let is_final_chunk = i == num_chunks - 1;
            let last_flag = commit_on_last && is_final_chunk;
            let frame = build_frame(
                cmd_byte,
                this_size as u8,
                addr,
                &payload[start..start + this_size],
                last_flag,
            );
            trace!(
                cmd = cmd_byte,
                chunk = i,
                addr,
                this_size,
                last_flag,
                "SET chunk"
            );
            self.write_output_report(&frame)?;
            let _ = self.read_response(cmd_byte, DEFAULT_TIMEOUT_MS);
        }
        Ok(())
    }

    /// Read the full 128-slot keymap (one chunked GET_KEY transaction).
    pub fn get_keymap(&self) -> Result<Keymap> {
        let payload = self.get_many(cmd::GET_KEY, KEYMAP_BYTES)?;
        Ok(Keymap::decode(&payload))
    }

    /// Read the FN-layer keymap (same shape as the base layer).
    pub fn get_fn_keymap(&self) -> Result<Keymap> {
        let payload = self.get_many(cmd::GET_FN_KEY, KEYMAP_BYTES)?;
        Ok(Keymap::decode(&payload))
    }

    /// Read the firmware's **factory-default** base keymap — what the user
    /// would get after a full reset, without actually issuing the reset.
    /// Used by the UI's "Reset to factory" button: stage these slots into
    /// the draft so the user can review before saving.
    pub fn get_default_keymap(&self) -> Result<Keymap> {
        let payload = self.get_many(cmd::GET_DEFAULT_KEY_MATRIX, KEYMAP_BYTES)?;
        Ok(Keymap::decode(&payload))
    }

    /// Read the firmware's factory-default Fn-layer keymap.
    pub fn get_default_fn_keymap(&self) -> Result<Keymap> {
        let payload = self.get_many(cmd::GET_DEFAULT_FN_KEY_MATRIX, KEYMAP_BYTES)?;
        Ok(Keymap::decode(&payload))
    }

    /// Write the full 128-slot keymap.
    pub fn set_keymap(&self, km: &Keymap) -> Result<()> {
        let payload = km.encode();
        info!(bytes = payload.len(), "SET_KEY");
        self.set_many(cmd::SET_KEY, &payload)
    }

    /// Write the FN-layer keymap.
    pub fn set_fn_keymap(&self, km: &Keymap) -> Result<()> {
        let payload = km.encode();
        info!(bytes = payload.len(), "SET_FN_KEY");
        self.set_many(cmd::SET_FN_KEY, &payload)
    }

    /// Apply a complete lighting configuration.
    pub fn set_lighting(&self, cfg: &LightingConfig) -> Result<()> {
        if self.info.protocol == DeviceProtocol::LegacyFeature {
            if cfg.mode == lighting::Mode::Custom {
                // Mode 80 renders the colour bank stored by set_custom_led.
                return crate::legacy_rgb::select_custom(
                    &self.device,
                    self.info.pid,
                    cfg.brightness.min(5),
                );
            }
            return crate::legacy::set_lighting(&self.device, self.info.pid, cfg);
        }
        let payload = lighting::led_effect_payload(cfg);
        let (r, g, b) = cfg.rgb();
        info!(
            mode = cfg.mode.name(),
            r, g, b,
            color_mode = cfg.color_mode,
            brightness = cfg.brightness,
            speed = cfg.speed,
            direction = ?cfg.direction,
            "SET_LED_EFFECT"
        );
        self.set(cmd::SET_LED_EFFECT, &payload)
    }

    /// Read every defined macro from the device.
    ///
    /// Walks the 400-byte index page, then for each non-empty slot pulls the
    /// 4-byte header and the per-action stream. Returns an `actions`-free
    /// macro for every slot that holds zero events on the device — callers
    /// can filter on `actions.is_empty()` if they want the strict list.
    pub fn get_macros(&self) -> Result<Vec<Macro>> {
        let index = self.get_many_at(cmd::GET_MACRO, 0, MACRO_INDEX_BYTES)?;
        let entries = macro_cmds::parse_index(&index);
        info!(slots_used = entries.len(), "GET_MACRO index");

        let mut macros = Vec::with_capacity(entries.len());
        for IndexEntry { macro_id, addr } in entries {
            let header = self.get_many_at(cmd::GET_MACRO, addr as u16, 4)?;
            if header.len() < 4 {
                continue;
            }
            let action_count = macro_cmds::parse_block_header(&header);
            let actions = if action_count > 0 {
                let bytes = self.get_many_at(
                    cmd::GET_MACRO,
                    addr.wrapping_add(4) as u16,
                    action_count * 4,
                )?;
                macro_cmds::parse_actions(&bytes, action_count)
            } else {
                Vec::new()
            };
            macros.push(Macro {
                macro_id,
                name: None,
                actions,
            });
        }
        Ok(macros)
    }

    /// Write a complete macro list to the device.
    ///
    /// Two-phase transaction: first the 400-byte index (no commit), then the
    /// concatenated data area at `addr = MACRO_DATA_ADDR` with the
    /// `last_packet` flag set on its final chunk.
    ///
    /// NB. To erase all macros, pass an empty slice — but note that the
    /// firmware needs at least one committed write for the change to stick;
    /// the no-data branch matches the AJAZZ driver verbatim and silently
    /// no-ops if there's nothing to write. For an "erase everything" path
    /// use `SET_FACTORY_RESET` with `MACRO_RESET = 4` (future work).
    pub fn set_macros(&self, macros: &[Macro]) -> Result<()> {
        let (index, data) = macro_cmds::encode_macros(macros)?;
        info!(
            slots = macros.iter().filter(|m| !m.actions.is_empty()).count(),
            data_bytes = data.len(),
            "SET_MACRO"
        );

        // Phase 1: write index (no commit yet).
        self.set_many_at(cmd::SET_MACRO, 0, &index, false)?;

        // Phase 2: write data area (commits the transaction).
        if !data.is_empty() {
            self.set_many_at(cmd::SET_MACRO, MACRO_DATA_ADDR, &data, true)?;
        }
        Ok(())
    }

    /// Switch which TFT animation the display plays back. The keyboard has
    /// several built-in animations plus one user slot. Calling this on the
    /// control endpoint (cmd 81) before `upload_tft_animation` is what makes
    /// the upload actually visible — otherwise the device keeps cycling
    /// through the previous built-in slot while our buffer sits unused.
    ///
    /// Valid index range is `0..deviceInfo.builtInCount` (built-ins) plus a
    /// vendor-specific value for the user slot. Empirically: try `0` first;
    /// if that selects a built-in animation, retry with values up to ~10
    /// until one shows your uploaded user frames. The AJAZZ web driver
    /// stores the active index in `_.value` (per-device runtime state) and
    /// passes whatever the UI's slot selector returned.
    pub fn set_tft_built_in_index(&self, index: u8) -> Result<()> {
        if self.info.protocol == DeviceProtocol::LegacyFeature {
            return Err(Error::NotImplemented(
                "Built-in animation selection is not verified for legacy firmware",
            ));
        }
        info!(index, "SET_TFT_BUILT_IN_INDEX");
        self.set(cmd::SET_TFT_BUILT_IN_INDEX, &[index])
    }

    /// Open the **TFT upload** interface — a different HID endpoint than the
    /// one returned by `open_control()`. The TFT path uses 4096-byte payload
    /// chunks (vs 56-byte chunks elsewhere), so it has its own HID interface
    /// on the device (`usage_page = 0xFF67` on the AK820 Pro, confirmed by
    /// inspecting the report descriptor with `probe_interfaces()` — the
    /// `0xFF67` collection advertises a 4104-byte output report).
    ///
    /// The feature-report profile instead uses FF13 for control and FF68
    /// (4096-byte reports) for TFT. An override can narrow the selected role,
    /// but cannot turn a keyboard or bootloader collection into a TFT endpoint.
    /// Callers can hold both `open_control()` and
    /// `open_tft()` connections concurrently — they're separate HID handles.
    pub fn open_tft() -> Result<Self> {
        let candidates = enumerate()?;
        let override_page = std::env::var("AK820_TFT_USAGE_PAGE")
            .ok()
            .and_then(|v| u16::from_str_radix(v.trim_start_matches("0x"), 16).ok());
        let eligible: Vec<_> = candidates
            .iter()
            .filter(|d| d.is_tft && override_page.is_none_or(|p| p == d.usage_page))
            .collect();
        if eligible.len() > 1 {
            return Err(Error::UnexpectedResponse(
                "Multiple TFT devices connected; leave only the target AK820 connected".into(),
            ));
        }
        let pick = eligible
            .first()
            .copied()
            .cloned()
            .ok_or(Error::DeviceNotFound {
                vid: VENDOR_ID,
                interface: -1,
            })?;
        let api = hid_api()?;
        let device = api.open_path(&std::ffi::CString::new(pick.path.clone()).unwrap())?;
        device.set_blocking_mode(true)?;
        let mut desc = [0u8; 4096];
        let n = device.get_report_descriptor(&mut desc)?;
        let expected = if pick.protocol == DeviceProtocol::LegacyFeature {
            4096
        } else {
            4104
        };
        if parse_max_output_report_size(&desc[..n]) != Some(expected) {
            return Err(Error::UnexpectedResponse(format!(
                "TFT descriptor does not declare the expected {expected}-byte output report"
            )));
        }
        info!(
            path = %pick.path,
            interface = pick.interface,
            usage_page = format!("0x{:04x}", pick.usage_page),
            "opened TFT upload interface"
        );
        Ok(Self { device, info: pick })
    }

    /// Upload a TFT animation as a single chunked transaction. Re-uses our
    /// existing `TftAnimation::encode()` for the 256-B-header + RGB565 stream
    /// and applies the bespoke 8-B per-chunk header — both decoded from the
    /// AJAZZ online driver.
    ///
    /// The chunk payload size is read from the device's HID descriptor (the
    /// `Items.reportCount` field in the JS source). For the AK820 Pro this
    /// is 4104 bytes per output report → 4096 bytes of payload after the
    /// 8-byte custom header.
    ///
    /// Must be called on a connection opened via `open_tft()` — the control
    /// interface's 64-byte report can't carry these chunks.
    pub fn upload_tft_animation(&self, anim: &TftAnimation) -> Result<()> {
        self.upload_tft_animation_confirmed(anim, false)
    }

    /// The legacy protocol replaces the stored animation before data transfer.
    /// Callers must obtain explicit consent before setting allow_legacy.
    pub fn upload_tft_animation_confirmed(
        &self,
        anim: &TftAnimation,
        allow_legacy: bool,
    ) -> Result<()> {
        if self.info.protocol == DeviceProtocol::LegacyFeature {
            if !allow_legacy {
                return Err(Error::NotImplemented("Legacy TFT upload needs confirmation: it replaces the stored animation. Use the TFT upload confirmation or CLI --allow-legacy."));
            }
            return crate::legacy_tft::upload_in_worker(&self.info, anim);
        }
        // Sanity-check the interface — if we're on the wrong one the
        // hidapi write will quietly fail with a frame-too-long error on macOS.
        if !self.info.is_tft {
            return Err(Error::UnexpectedResponse(
                "upload_tft_animation needs a TFT endpoint; use Connection::open_tft()".into(),
            ));
        }

        let payload = anim.encode()?;
        const HEADER_LEN: usize = 8;
        const TFT_REPORT_LEN: usize = 4104;
        const TFT_PAYLOAD_LEN: usize = TFT_REPORT_LEN - HEADER_LEN; // 4096

        let total_chunks = payload.len().div_ceil(TFT_PAYLOAD_LEN).max(1);
        if total_chunks > u16::MAX as usize {
            return Err(Error::OutOfRange {
                field: "tft chunk count",
                value: total_chunks as i64,
                max: u16::MAX as i64,
            });
        }

        info!(
            bytes = payload.len(),
            chunks = total_chunks,
            "SET_TFT_USER_ANIMATION upload"
        );

        // Pre-allocated per-chunk buffer: 1 report-id byte + 8 header + 4096 payload.
        let mut report = vec![0u8; 1 + TFT_REPORT_LEN];
        for i in 0..total_chunks {
            let start = i * TFT_PAYLOAD_LEN;
            let end = (start + TFT_PAYLOAD_LEN).min(payload.len());
            let header =
                build_tft_header(cmd::SET_TFT_USER_ANIMATION, i as u16, total_chunks as u16);

            // Reset payload region to zero (header overwrites bytes 1..9).
            for b in &mut report[1..] {
                *b = 0;
            }
            report[0] = REPORT_ID;
            report[1..1 + HEADER_LEN].copy_from_slice(&header);
            let dst = &mut report[1 + HEADER_LEN..1 + HEADER_LEN + (end - start)];
            dst.copy_from_slice(&payload[start..end]);

            trace!(chunk = i, total = total_chunks, "TFT TX");
            self.device.write(&report)?;
        }
        Ok(())
    }

    /// Read the 128-LED per-key colour map.
    pub fn get_custom_led(&self) -> Result<CustomLedMap> {
        if self.info.protocol == DeviceProtocol::LegacyFeature {
            return Err(Error::NotImplemented(
                "Feature-report firmware has no colour-bank read-back",
            ));
        }
        let payload = self.get_many_at(cmd::GET_CUSTOM_LED_DATA, 0, CUSTOM_LED_BYTES)?;
        Ok(CustomLedMap::decode(&payload))
    }

    /// Write the 128-LED per-key colour map. Caller is responsible for
    /// switching the active lighting effect to one that reads from this
    /// buffer (the standard 20 modes ignore it).
    pub fn set_custom_led(&self, map: &CustomLedMap) -> Result<()> {
        if self.info.protocol == DeviceProtocol::LegacyFeature {
            // ANSI slot table; each write is saved, so not for streaming.
            return crate::legacy_rgb::write_bank(&self.device, self.info.pid, map);
        }
        let payload = map.encode();
        info!(bytes = payload.len(), "SET_CUSTOM_LED_DATA");
        self.set_many_at(cmd::SET_CUSTOM_LED_DATA, 0, &payload, true)
    }

    /// Read the older firmware's battery status without synthesizing modern fields.
    pub fn get_legacy_battery(&self) -> Result<crate::legacy::BatteryStatus> {
        if self.info.protocol != DeviceProtocol::LegacyFeature {
            return Err(Error::NotImplemented(
                "Use get_device_info for web-output firmware",
            ));
        }
        crate::legacy::battery(&self.device, self.info.pid)
    }

    /// Set a legacy sleep timer in minutes (0, 1, 5, 30). No read-back format is known.
    pub fn set_legacy_sleep(&self, minutes: u8) -> Result<()> {
        if self.info.protocol != DeviceProtocol::LegacyFeature {
            return Err(Error::NotImplemented(
                "Use set_game_mode for web-output firmware",
            ));
        }
        crate::legacy::set_sleep(&self.device, self.info.pid, minutes)
    }

    /// Synchronize the older firmware clock to an explicitly supplied local time.
    pub fn sync_legacy_clock(&self, now: time::OffsetDateTime) -> Result<()> {
        if self.info.protocol != DeviceProtocol::LegacyFeature {
            return Err(Error::NotImplemented(
                "Clock sync is not decoded for web-output firmware",
            ));
        }
        crate::legacy::sync_clock(&self.device, self.info.pid, now)
    }

    /// Read the device-info struct (firmware, battery, profile, …).
    pub fn get_device_info(&self) -> Result<DeviceInfoReport> {
        let payload = self.get(cmd::GET_DEVICE_INFO, 48)?;
        Ok(DeviceInfoReport::parse(&payload))
    }

    /// Read the game-mode struct (sleep timer, key delay, report rate, …).
    pub fn get_game_mode(&self) -> Result<GameMode> {
        let payload = self.get(cmd::GET_GAME_MODE, 56)?;
        Ok(GameMode::parse(&payload))
    }

    /// Write the game-mode struct. Sends all 56 bytes — callers should
    /// usually `get_game_mode` first, mutate one field, then `set_game_mode`.
    pub fn set_game_mode(&self, gm: &GameMode) -> Result<()> {
        let payload = gm.serialize();
        info!(
            sleep_time = gm.sleep_time,
            game_mode = gm.game_mode,
            "SET_GAME_MODE"
        );
        self.set(cmd::SET_GAME_MODE, &payload)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeReport {
    pub connected: bool,
    pub interface: i32,
    pub product: Option<String>,
    pub firmware_version: Option<String>,
}

#[cfg(test)]
mod detection_tests {
    use super::*;

    fn endpoint(page: u16) -> DeviceInfo {
        DeviceInfo {
            protocol: DeviceProtocol::WebOutput,
            is_control: false,
            is_tft: false,
            vid: VENDOR_ID,
            pid: 0x800A,
            interface: -1,
            usage_page: page,
            usage: 1,
            manufacturer: Some("SONiX".into()),
            product: Some("AK820".into()),
            serial: None,
            path: format!("test:{page}"),
        }
    }

    #[test]
    fn ff13_changes_ff68_from_control_to_screen_role() {
        let mut devices = vec![
            endpoint(0x0001),
            endpoint(0x000c),
            endpoint(0xff68),
            endpoint(0xff13),
        ];
        classify_candidates(&mut devices);
        assert!(!devices[0].is_control && !devices[0].is_tft);
        assert!(!devices[1].is_control && !devices[1].is_tft);
        assert!(devices[2].is_tft && !devices[2].is_control);
        assert!(devices[3].is_control && !devices[3].is_tft);
        assert_eq!(devices[3].protocol, DeviceProtocol::LegacyFeature);
    }

    #[test]
    fn web_firmware_keeps_original_endpoint_roles() {
        let mut devices = vec![endpoint(0xff68), endpoint(0xff67)];
        classify_candidates(&mut devices);
        assert!(devices[0].is_control && !devices[0].is_tft);
        assert!(devices[1].is_tft && !devices[1].is_control);
        assert_eq!(devices[0].protocol, DeviceProtocol::WebOutput);
        assert!(PRODUCT_IDS.contains(&0x800A));
        assert!(!PRODUCT_IDS.contains(&0x7140));
    }

    #[test]
    fn web_usage_pages_and_interface_fallback_preserve_existing_selection() {
        for page in CONTROL_PAGES {
            let mut devices = vec![endpoint(*page), endpoint(0xff67)];
            classify_candidates(&mut devices);
            assert!(devices[0].is_control);
            assert!(!devices[1].is_control);
        }
        let mut fallback = endpoint(0xff99);
        fallback.interface = CONTROL_INTERFACE;
        let mut devices = vec![fallback];
        classify_candidates(&mut devices);
        assert!(devices[0].is_control);
        devices.push(endpoint(0xff68));
        classify_candidates(&mut devices);
        assert!(!devices[0].is_control);
        assert!(devices[1].is_control);
    }
    #[test]
    fn hardware_screen_descriptor_is_4096_bytes_without_report_id() {
        let desc =
            hex::decode("0668ff0961a1010962150026ff009540750881020963150026ff0096001075089102c0")
                .unwrap();
        assert_eq!(parse_max_output_report_size(&desc), Some(4096));
    }
}
