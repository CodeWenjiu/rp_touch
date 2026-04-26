use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use rp_telemetry::TelemetryFrame;
use serde::Serialize;
use serde_json::Value;
use serialport::{SerialPortInfo, SerialPortType};
use tauri::{AppHandle, Emitter, State};

const RP_TOUCH_USB_VID: u16 = 0xc0de;
const RP_TOUCH_USB_PID: u16 = 0xcafe;
const SERIAL_BAUDRATE: u32 = 115_200;
const SERIAL_TIMEOUT_MS: u64 = 50;
const MAX_SERIAL_LINE_LEN: usize = 128;
const TELEMETRY_EVENT: &str = "telemetry-angle";
const RAD_TO_DEG: f32 = 57.295_78_f32;
const DEG_TO_RAD: f32 = 0.017_453_292_f32;
const ACCEL_LSB_PER_G_8G: f32 = 4096.0_f32;
const GYRO_LSB_PER_DPS_512: f32 = 64.0_f32;
const FILTER_DT_FALLBACK_S: f32 = 0.02_f32;
const FILTER_DT_MIN_S: f32 = 0.001_f32;
const FILTER_DT_MAX_S: f32 = 0.1_f32;
const ACCEL_CORRECTION_GAIN: f32 = 0.9_f32;
const ACCEL_TRUST_MIN_G: f32 = 0.75_f32;
const ACCEL_TRUST_MAX_G: f32 = 1.25_f32;
const WORLD_UP_DISPLAY: [f32; 3] = [0.0, 1.0, 0.0];
const VIEW_FRAME_FIX_QUAT: [f32; 4] = [0.0, 1.0, 0.0, 0.0];
const MODEL_BUILD_REL: &str = "model/build/rp_touch";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelPayload {
    model_name: String,
    source_file: String,
    gltf: String,
    parts: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryAnglePayload {
    pitch_deg: f32,
    roll_deg: f32,
    quat_w: f32,
    quat_x: f32,
    quat_y: f32,
    quat_z: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SerialConnectionPayload {
    connected: bool,
    port_name: Option<String>,
}

struct SerialWorker {
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
    port_name: String,
}

struct SerialBridgeState {
    worker: Mutex<Option<SerialWorker>>,
    reset_heading_requested: Arc<AtomicBool>,
}

impl Default for SerialBridgeState {
    fn default() -> Self {
        Self {
            worker: Mutex::new(None),
            reset_heading_requested: Arc::new(AtomicBool::new(false)),
        }
    }
}

struct TiltEstimate {
    pitch_deg: f32,
    roll_deg: f32,
    quat_w: f32,
    quat_x: f32,
    quat_y: f32,
    quat_z: f32,
}

struct TiltComplementaryFilter {
    has_value: bool,
    orientation_world_to_display: [f32; 4],
    last_update: Option<Instant>,
}

impl Default for TiltComplementaryFilter {
    fn default() -> Self {
        Self {
            has_value: false,
            orientation_world_to_display: [1.0, 0.0, 0.0, 0.0],
            last_update: None,
        }
    }
}

impl TiltComplementaryFilter {
    fn update(
        &mut self,
        accel_raw: [i16; 3],
        gyro_raw: [i16; 3],
        reset_heading: bool,
    ) -> TiltEstimate {
        let now = Instant::now();
        let dt = self
            .last_update
            .map(|prev| (now - prev).as_secs_f32())
            .unwrap_or(FILTER_DT_FALLBACK_S)
            .clamp(FILTER_DT_MIN_S, FILTER_DT_MAX_S);
        self.last_update = Some(now);

        let accel_display = accel_raw_to_display_g(accel_raw);
        let accel_mag = vec3_len(accel_display);
        let accel_unit = vec3_normalize(accel_display);

        if !self.has_value {
            self.orientation_world_to_display =
                quat_from_to(WORLD_UP_DISPLAY, accel_unit.unwrap_or(WORLD_UP_DISPLAY));
            self.has_value = true;
        }

        let mut omega_display = gyro_raw_to_display_rad_s(gyro_raw);

        if (ACCEL_TRUST_MIN_G..=ACCEL_TRUST_MAX_G).contains(&accel_mag) {
            if let Some(accel_unit) = accel_unit {
                let predicted_gravity = quat_rotate_vector(
                    self.orientation_world_to_display,
                    WORLD_UP_DISPLAY,
                );
                let correction_axis = vec3_cross(predicted_gravity, accel_unit);
                omega_display = vec3_add(
                    omega_display,
                    vec3_scale(correction_axis, ACCEL_CORRECTION_GAIN),
                );
            }
        }

        let previous_orientation = self.orientation_world_to_display;
        let delta = quat_normalize([
            1.0,
            0.5 * omega_display[0] * dt,
            0.5 * omega_display[1] * dt,
            0.5 * omega_display[2] * dt,
        ]);

        let mut orientation = quat_normalize(quat_mul(delta, previous_orientation));
        if quat_dot(orientation, previous_orientation) < 0.0 {
            orientation = quat_neg(orientation);
        }
        self.orientation_world_to_display = orientation;
        let mut orientation_for_view = quat_frame_transform(orientation, VIEW_FRAME_FIX_QUAT);

        if reset_heading {
            if let Some(correction) = heading_recenter_quat(orientation_for_view) {
                orientation_for_view = quat_normalize(quat_mul(correction, orientation_for_view));
                self.orientation_world_to_display =
                    quat_frame_transform(orientation_for_view, VIEW_FRAME_FIX_QUAT);
            }
        }

        let gravity_display = quat_rotate_vector(orientation_for_view, WORLD_UP_DISPLAY);

        let pitch_deg = (-gravity_display[2]).atan2(
            (gravity_display[0] * gravity_display[0] + gravity_display[1] * gravity_display[1])
                .sqrt(),
        ) * RAD_TO_DEG;
        let roll_deg = gravity_display[0].atan2(gravity_display[1]) * RAD_TO_DEG;

        TiltEstimate {
            pitch_deg,
            roll_deg: normalize_angle_deg(roll_deg),
            quat_w: orientation_for_view[0],
            quat_x: orientation_for_view[1],
            quat_y: orientation_for_view[2],
            quat_z: orientation_for_view[3],
        }
    }
}

#[tauri::command]
fn load_rp_touch_model() -> Result<ModelPayload, String> {
    let workspace_root = workspace_root()?;
    let gltf_path = ensure_model_build_ready(&workspace_root)?;

    let gltf_text = fs::read_to_string(&gltf_path)
        .map_err(|err| format!("failed to read model file '{}': {err}", gltf_path.display()))?;
    let mut gltf_json: Value = serde_json::from_str(&gltf_text).map_err(|err| {
        format!(
            "failed to parse glTF json from '{}': {err}",
            gltf_path.display()
        )
    })?;

    inline_first_buffer(&mut gltf_json, &gltf_path)?;

    let parts = collect_part_labels(&workspace_root.join("model").join("src"));
    let parts = if parts.is_empty() {
        vec![
            "RP Touch".to_string(),
            "Screen".to_string(),
            "PCB Assembly".to_string(),
        ]
    } else {
        parts
    };

    let gltf = serde_json::to_string(&gltf_json)
        .map_err(|err| format!("failed to serialize glTF json: {err}"))?;

    Ok(ModelPayload {
        model_name: "rp_touch".to_string(),
        source_file: gltf_path.display().to_string(),
        gltf,
        parts,
    })
}

#[tauri::command]
fn list_serial_ports() -> Result<Vec<String>, String> {
    let mut ports = serialport::available_ports()
        .map_err(|err| format!("failed to enumerate serial ports: {err}"))?
        .into_iter()
        .map(|port| port.port_name)
        .collect::<Vec<_>>();
    ports.sort();
    Ok(ports)
}

#[tauri::command]
fn serial_connection_state(
    state: State<'_, SerialBridgeState>,
) -> Result<SerialConnectionPayload, String> {
    let mut worker_slot = state
        .worker
        .lock()
        .map_err(|_| "serial bridge state poisoned".to_string())?;

    cleanup_finished_worker(&mut worker_slot);

    let payload = if let Some(worker) = worker_slot.as_ref() {
        SerialConnectionPayload {
            connected: true,
            port_name: Some(worker.port_name.clone()),
        }
    } else {
        SerialConnectionPayload {
            connected: false,
            port_name: None,
        }
    };

    Ok(payload)
}

#[tauri::command]
fn connect_serial(
    app: AppHandle,
    state: State<'_, SerialBridgeState>,
    port: Option<String>,
) -> Result<String, String> {
    let mut worker_slot = state
        .worker
        .lock()
        .map_err(|_| "serial bridge state poisoned".to_string())?;

    cleanup_finished_worker(&mut worker_slot);

    if let Some(worker) = worker_slot.as_ref() {
        return Err(format!(
            "serial stream is already running on '{}'",
            worker.port_name
        ));
    }

    let port_name = resolve_serial_port(port)?;
    let serial = serialport::new(&port_name, SERIAL_BAUDRATE)
        .timeout(Duration::from_millis(SERIAL_TIMEOUT_MS))
        .open()
        .map_err(|err| format!("failed to open serial port '{}': {err}", port_name))?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let reset_heading_flag = Arc::clone(&state.reset_heading_requested);
    let thread_port_name = port_name.clone();
    let app_handle = app.clone();

    let handle = thread::Builder::new()
        .name("rp-touch-serial".to_string())
        .spawn(move || run_serial_loop(app_handle, serial, stop_flag, reset_heading_flag))
        .map_err(|err| format!("failed to spawn serial worker: {err}"))?;

    *worker_slot = Some(SerialWorker {
        stop,
        handle,
        port_name: thread_port_name,
    });

    Ok(port_name)
}

#[tauri::command]
fn disconnect_serial(state: State<'_, SerialBridgeState>) -> Result<(), String> {
    let mut worker_slot = state
        .worker
        .lock()
        .map_err(|_| "serial bridge state poisoned".to_string())?;

    cleanup_finished_worker(&mut worker_slot);

    if let Some(worker) = worker_slot.take() {
        worker.stop.store(true, Ordering::Relaxed);
        let _ = worker.handle.join();
    }
    state.reset_heading_requested.store(false, Ordering::Relaxed);

    Ok(())
}

#[tauri::command]
fn reset_heading(state: State<'_, SerialBridgeState>) -> Result<(), String> {
    let mut worker_slot = state
        .worker
        .lock()
        .map_err(|_| "serial bridge state poisoned".to_string())?;

    cleanup_finished_worker(&mut worker_slot);

    if worker_slot.is_none() {
        return Err("serial stream is not running".to_string());
    }

    state.reset_heading_requested.store(true, Ordering::Relaxed);
    Ok(())
}

fn cleanup_finished_worker(slot: &mut Option<SerialWorker>) {
    let is_finished = slot
        .as_ref()
        .is_some_and(|worker| worker.handle.is_finished());
    if is_finished {
        if let Some(worker) = slot.take() {
            let _ = worker.handle.join();
        }
    }
}

fn resolve_serial_port(port: Option<String>) -> Result<String, String> {
    let ports = serialport::available_ports()
        .map_err(|err| format!("failed to enumerate serial ports: {err}"))?;

    if ports.is_empty() {
        return Err("no serial ports available".to_string());
    }

    if let Some(requested) = port {
        if ports.iter().any(|info| info.port_name == requested) {
            return Ok(requested);
        }
        return Err(format!("serial port '{}' is not available", requested));
    }

    select_default_port(&ports).ok_or_else(|| "unable to auto-select serial port".to_string())
}

fn select_default_port(ports: &[SerialPortInfo]) -> Option<String> {
    ports
        .iter()
        .find_map(|info| match &info.port_type {
            SerialPortType::UsbPort(usb)
                if usb.vid == RP_TOUCH_USB_VID && usb.pid == RP_TOUCH_USB_PID =>
            {
                Some(info.port_name.clone())
            }
            _ => None,
        })
        .or_else(|| ports.first().map(|info| info.port_name.clone()))
}

fn run_serial_loop(
    app: AppHandle,
    mut serial: Box<dyn serialport::SerialPort>,
    stop: Arc<AtomicBool>,
    reset_heading_requested: Arc<AtomicBool>,
) {
    let mut read_buf = [0u8; 256];
    let mut line_buf = String::new();
    let mut filter = TiltComplementaryFilter::default();

    while !stop.load(Ordering::Relaxed) {
        match serial.read(&mut read_buf) {
            Ok(count) if count > 0 => process_serial_chunk(
                &app,
                &mut line_buf,
                &read_buf[..count],
                &mut filter,
                &reset_heading_requested,
            ),
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::TimedOut => {}
            Err(_) => break,
        }
    }
}

fn process_serial_chunk(
    app: &AppHandle,
    line_buf: &mut String,
    chunk: &[u8],
    filter: &mut TiltComplementaryFilter,
    reset_heading_requested: &Arc<AtomicBool>,
) {
    for &byte in chunk {
        match byte {
            b'\n' => {
                if let Ok(frame) = TelemetryFrame::deformat(line_buf) {
                    let should_reset_heading =
                        reset_heading_requested.swap(false, Ordering::Relaxed);
                    let estimate = filter.update(frame.accel, frame.gyro, should_reset_heading);
                    if telemetry_is_finite(&estimate) {
                        let payload = TelemetryAnglePayload {
                            pitch_deg: estimate.pitch_deg,
                            roll_deg: estimate.roll_deg,
                            quat_w: estimate.quat_w,
                            quat_x: estimate.quat_x,
                            quat_y: estimate.quat_y,
                            quat_z: estimate.quat_z,
                        };
                        let _ = app.emit(TELEMETRY_EVENT, payload);
                    }
                }
                line_buf.clear();
            }
            b'\r' => {}
            _ => {
                if line_buf.len() >= MAX_SERIAL_LINE_LEN {
                    line_buf.clear();
                    continue;
                }
                if byte.is_ascii() {
                    line_buf.push(byte as char);
                }
            }
        }
    }
}

fn normalize_angle_deg(angle: f32) -> f32 {
    if !angle.is_finite() {
        return 0.0;
    }
    let mut out = angle;
    while out <= -180.0 {
        out += 360.0;
    }
    while out > 180.0 {
        out -= 360.0;
    }
    out
}

fn accel_raw_to_display_g(accel: [i16; 3]) -> [f32; 3] {
    let sensor = accel_raw_to_g_sensor(accel);
    sensor_to_display(mount_compensate_sensor(sensor))
}

fn accel_raw_to_g_sensor(accel: [i16; 3]) -> [f32; 3] {
    [
        accel[0] as f32 / ACCEL_LSB_PER_G_8G,
        accel[1] as f32 / ACCEL_LSB_PER_G_8G,
        accel[2] as f32 / ACCEL_LSB_PER_G_8G,
    ]
}

fn gyro_raw_to_display_rad_s(gyro: [i16; 3]) -> [f32; 3] {
    let sensor = gyro_raw_to_rad_s_sensor(gyro);
    sensor_to_display(mount_compensate_sensor(sensor))
}

fn gyro_raw_to_rad_s_sensor(gyro: [i16; 3]) -> [f32; 3] {
    [
        gyro[0] as f32 / GYRO_LSB_PER_DPS_512 * DEG_TO_RAD,
        gyro[1] as f32 / GYRO_LSB_PER_DPS_512 * DEG_TO_RAD,
        gyro[2] as f32 / GYRO_LSB_PER_DPS_512 * DEG_TO_RAD,
    ]
}

fn mount_compensate_sensor(v: [f32; 3]) -> [f32; 3] {
    // IMU is mounted on the PCB back side: flip sensor X so host pitch matches board pitch.
    [-v[0], v[1], v[2]]
}

fn sensor_to_display(v: [f32; 3]) -> [f32; 3] {
    // Display frame uses X<-sensor Y, Y<-sensor Z, Z<-sensor X.
    [v[1], v[2], v[0]]
}

fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn vec3_len(v: [f32; 3]) -> f32 {
    vec3_dot(v, v).sqrt()
}

fn vec3_scale(v: [f32; 3], scale: f32) -> [f32; 3] {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}

fn vec3_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn vec3_normalize(v: [f32; 3]) -> Option<[f32; 3]> {
    let len = vec3_len(v);
    if len <= 1.0e-6 {
        None
    } else {
        Some([v[0] / len, v[1] / len, v[2] / len])
    }
}

fn quat_from_to(from: [f32; 3], to: [f32; 3]) -> [f32; 4] {
    let from = vec3_normalize(from).unwrap_or([0.0, 1.0, 0.0]);
    let to = vec3_normalize(to).unwrap_or(from);
    let dot = vec3_dot(from, to).clamp(-1.0, 1.0);

    if dot > 0.999_999 {
        return [1.0, 0.0, 0.0, 0.0];
    }

    if dot < -0.999_999 {
        let basis = if from[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let axis = vec3_normalize(vec3_cross(from, basis)).unwrap_or([0.0, 0.0, 1.0]);
        // 180deg axis-angle -> w=0, xyz=axis.
        return [0.0, axis[0], axis[1], axis[2]];
    }

    let cross = vec3_cross(from, to);
    quat_normalize([1.0 + dot, cross[0], cross[1], cross[2]])
}

fn heading_recenter_quat(orientation_for_view: [f32; 4]) -> Option<[f32; 4]> {
    const WORLD_FORWARD_DISPLAY: [f32; 3] = [0.0, 0.0, 1.0];

    let gravity = vec3_normalize(quat_rotate_vector(orientation_for_view, WORLD_UP_DISPLAY))?;
    let forward = quat_rotate_vector(orientation_for_view, WORLD_FORWARD_DISPLAY);

    let forward_plane = vec3_normalize(vec3_project_on_plane(forward, gravity))?;
    let mut reference_plane = vec3_normalize(vec3_project_on_plane(WORLD_FORWARD_DISPLAY, gravity));
    if reference_plane.is_none() {
        reference_plane = vec3_normalize(vec3_project_on_plane([1.0, 0.0, 0.0], gravity));
    }
    let reference_plane = reference_plane?;

    let sin_angle = vec3_dot(vec3_cross(forward_plane, reference_plane), gravity);
    let cos_angle = vec3_dot(forward_plane, reference_plane).clamp(-1.0, 1.0);
    let angle = sin_angle.atan2(cos_angle);
    Some(quat_from_axis_angle(gravity, angle))
}

fn quat_dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

fn quat_frame_transform(q: [f32; 4], basis_fix: [f32; 4]) -> [f32; 4] {
    // Expresses the same physical orientation in a corrected frame:
    // q' = C * q * C^-1
    quat_normalize(quat_mul(quat_mul(basis_fix, q), quat_conjugate(basis_fix)))
}

fn quat_neg(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], -q[3]]
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

fn quat_conjugate(q: [f32; 4]) -> [f32; 4] {
    [q[0], -q[1], -q[2], -q[3]]
}

fn quat_from_axis_angle(axis: [f32; 3], angle_rad: f32) -> [f32; 4] {
    let axis = vec3_normalize(axis).unwrap_or([0.0, 1.0, 0.0]);
    let half = 0.5 * angle_rad;
    let (sin_half, cos_half) = half.sin_cos();
    quat_normalize([
        cos_half,
        axis[0] * sin_half,
        axis[1] * sin_half,
        axis[2] * sin_half,
    ])
}

fn quat_rotate_vector(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let p = [0.0, v[0], v[1], v[2]];
    let rotated = quat_mul(quat_mul(q, p), quat_conjugate(q));
    [rotated[1], rotated[2], rotated[3]]
}

fn vec3_project_on_plane(v: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
    vec3_add(v, vec3_scale(normal, -vec3_dot(v, normal)))
}

fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if !len.is_finite() || len <= 1.0e-6 {
        [1.0, 0.0, 0.0, 0.0]
    } else {
        [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
    }
}

fn telemetry_is_finite(v: &TiltEstimate) -> bool {
    v.pitch_deg.is_finite()
        && v.roll_deg.is_finite()
        && v.quat_w.is_finite()
        && v.quat_x.is_finite()
        && v.quat_y.is_finite()
        && v.quat_z.is_finite()
}

fn workspace_root() -> Result<PathBuf, String> {
    // CARGO_MANIFEST_DIR points to .../tools/rp_touch_host/src-tauri
    let base = Path::new(env!("CARGO_MANIFEST_DIR"));
    base.ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve workspace root".to_string())
}

fn ensure_model_build_ready(workspace_root: &Path) -> Result<PathBuf, String> {
    let model_root = workspace_root.join("model");
    let gltf_path = workspace_root.join(MODEL_BUILD_REL);
    let bin_path = gltf_path.with_extension("bin");

    let should_export = !gltf_path.is_file()
        || !bin_path.is_file()
        || is_model_build_stale(&model_root, &gltf_path, &bin_path)?;

    if should_export {
        run_model_export(&model_root)?;
    }

    if gltf_path.is_file() && bin_path.is_file() {
        Ok(gltf_path)
    } else {
        Err(format!(
            "model export did not produce expected files '{}' and '{}'",
            gltf_path.display(),
            bin_path.display()
        ))
    }
}

fn is_model_build_stale(model_root: &Path, gltf_path: &Path, bin_path: &Path) -> Result<bool, String> {
    let newest_source = newest_model_source_mtime(model_root)?;
    let gltf_mtime = file_mtime(gltf_path)?;
    let bin_mtime = file_mtime(bin_path)?;
    let oldest_output = if gltf_mtime <= bin_mtime {
        gltf_mtime
    } else {
        bin_mtime
    };
    Ok(newest_source > oldest_output)
}

fn newest_model_source_mtime(model_root: &Path) -> Result<SystemTime, String> {
    let src_root = model_root.join("src");
    let mut newest = SystemTime::UNIX_EPOCH;

    collect_newest_python_mtime(&src_root, &mut newest)?;

    let pyproject = model_root.join("pyproject.toml");
    if pyproject.is_file() {
        let t = file_mtime(&pyproject)?;
        if t > newest {
            newest = t;
        }
    }

    Ok(newest)
}

fn collect_newest_python_mtime(dir: &Path, newest: &mut SystemTime) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|err| format!("failed to read model source dir '{}': {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("failed to read model source entry: {err}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("failed to inspect '{}': {err}", path.display()))?;
        if file_type.is_dir() {
            collect_newest_python_mtime(&path, newest)?;
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "py") {
            let t = file_mtime(&path)?;
            if t > *newest {
                *newest = t;
            }
        }
    }
    Ok(())
}

fn file_mtime(path: &Path) -> Result<SystemTime, String> {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .map_err(|err| format!("failed to read modified time for '{}': {err}", path.display()))
}

fn run_model_export(model_root: &Path) -> Result<(), String> {
    let uv_result = Command::new("uv")
        .args(["run", "src/main.py", "--out", "build/rp_touch"])
        .current_dir(model_root)
        .output();

    match uv_result {
        Ok(output) if output.status.success() => return Ok(()),
        Ok(output) => {
            let uv_error = format_command_error("uv run src/main.py --out build/rp_touch", &output);
            let just_result = Command::new("just")
                .args(["export"])
                .current_dir(model_root)
                .output();
            return match just_result {
                Ok(just_output) if just_output.status.success() => Ok(()),
                Ok(just_output) => {
                    let just_error = format_command_error("just export", &just_output);
                    Err(format!(
                        "auto model export failed.\nPrimary: {uv_error}\nFallback: {just_error}"
                    ))
                }
                Err(err) => Err(format!(
                    "auto model export failed.\nPrimary: {uv_error}\nFallback command launch failed: {err}"
                )),
            };
        }
        Err(err) => {
            let just_result = Command::new("just")
                .args(["export"])
                .current_dir(model_root)
                .output();
            return match just_result {
                Ok(just_output) if just_output.status.success() => Ok(()),
                Ok(just_output) => {
                    let just_error = format_command_error("just export", &just_output);
                    Err(format!(
                        "failed to launch uv exporter: {err}. fallback also failed: {just_error}"
                    ))
                }
                Err(just_err) => Err(format!(
                    "failed to launch uv exporter: {err}. fallback launch failed: {just_err}"
                )),
            };
        }
    }
}

fn format_command_error(cmd: &str, output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    format!(
        "'{cmd}' exited with status {}. stdout: {} stderr: {}",
        output
            .status
            .code()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        if stdout.is_empty() { "<empty>" } else { stdout.as_str() },
        if stderr.is_empty() { "<empty>" } else { stderr.as_str() },
    )
}

fn inline_first_buffer(gltf_json: &mut Value, gltf_path: &Path) -> Result<(), String> {
    let buffers = gltf_json
        .get_mut("buffers")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "glTF file does not contain a valid 'buffers' array".to_string())?;

    let first_buffer = buffers
        .first_mut()
        .ok_or_else(|| "glTF file does not contain any buffers".to_string())?;

    let uri = first_buffer
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(|| "glTF first buffer has no string 'uri'".to_string())?;

    if uri.starts_with("data:") {
        return Ok(());
    }

    let bin_path = gltf_path.with_file_name(uri);
    let bin = fs::read(&bin_path).map_err(|err| {
        format!(
            "failed to read model binary '{}': {err}",
            bin_path.display()
        )
    })?;

    let encoded = base64_encode(&bin);
    let data_uri = format!("data:application/octet-stream;base64,{encoded}");
    first_buffer["uri"] = Value::String(data_uri);

    Ok(())
}

fn collect_part_labels(model_src_dir: &Path) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut labels = Vec::new();

    if let Ok(entries) = fs::read_dir(model_src_dir) {
        let mut files = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "py"))
            .collect::<Vec<_>>();
        files.sort();

        for path in files {
            if let Ok(content) = fs::read_to_string(path) {
                extract_labels(&content, &mut seen, &mut labels);
            }
        }
    }

    labels
}

fn extract_labels(source: &str, seen: &mut HashSet<String>, out: &mut Vec<String>) {
    for marker in ["label = \"", "label=\""] {
        let mut remainder = source;
        while let Some(idx) = remainder.find(marker) {
            let start = idx + marker.len();
            let tail = &remainder[start..];
            if let Some(end) = tail.find('"') {
                let value = tail[..end].trim();
                if !value.is_empty() {
                    let label = value.to_string();
                    if seen.insert(label.clone()) {
                        out.push(label);
                    }
                }
                remainder = &tail[end + 1..];
            } else {
                break;
            }
        }
    }
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);

    let mut i = 0;
    while i + 3 <= input.len() {
        let b0 = input[i];
        let b1 = input[i + 1];
        let b2 = input[i + 2];
        i += 3;

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
    }

    let rem = input.len() - i;
    if rem == 1 {
        let b0 = input[i];
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[((b0 & 0b0000_0011) << 4) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let b0 = input[i];
        let b1 = input[i + 1];
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        out.push(TABLE[((b1 & 0b0000_1111) << 2) as usize] as char);
        out.push('=');
    }

    out
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SerialBridgeState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            load_rp_touch_model,
            list_serial_ports,
            serial_connection_state,
            connect_serial,
            disconnect_serial,
            reset_heading
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
