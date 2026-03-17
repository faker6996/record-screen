#![allow(dead_code)]

use std::{
    collections::HashMap,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    sync::mpsc,
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

use capture::CaptureError;
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::{OwnedFd, OwnedObjectPath, OwnedValue, Str},
};

const SCREENCAST_DESTINATION: &str = "org.freedesktop.portal.Desktop";
const SCREENCAST_OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
const SCREENCAST_INTERFACE: &str = "org.freedesktop.portal.ScreenCast";
const PORTAL_LISTENER_READY_TIMEOUT: Duration = Duration::from_secs(5);
const PORTAL_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
static CURRENT_RESTORE_TOKEN: OnceLock<Mutex<Option<String>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenCastPortalCapabilities {
    pub available_source_types: u32,
    pub available_cursor_modes: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenCastPortalStartResult {
    pub session_handle: String,
    pub restore_token: Option<String>,
    pub stream_node_ids: Vec<u32>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ScreenCastPortalRuntimeSession {
    pub session_handle: String,
    pub restore_token: Option<String>,
    pub stream_node_ids: Vec<u32>,
    pub stream_target_ids: Vec<String>,
    pub pipewire_remote_fd: OwnedFd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenCastPortalProbe {
    Available(ScreenCastPortalCapabilities),
    MissingPortal,
    MissingDbusTools,
    Unreachable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeWireGstreamerSupport {
    Available,
    Missing,
    Unknown,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenCastPortalCommandPlan {
    pub create_session: Vec<String>,
    pub select_sources: Vec<String>,
    pub start: Vec<String>,
    pub open_pipewire_remote: Vec<String>,
}

pub fn start_unavailable(wayland_display: &str) -> CaptureError {
    let gstreamer_pipewire = gstreamer_pipewire_support();
    let guidance = match probe_screen_cast_portal() {
        ScreenCastPortalProbe::Available(capabilities) => format!(
            "Wayland session {wayland_display} was detected without XWayland. The ScreenCast portal is available ({}, {}). {}",
            describe_source_types(capabilities.available_source_types),
            describe_cursor_modes(capabilities.available_cursor_modes),
            pipewire_support_copy(gstreamer_pipewire),
        ),
        ScreenCastPortalProbe::MissingPortal => format!(
            "Wayland session {wayland_display} was detected without XWayland, and no ScreenCast portal could be reached. Install xdg-desktop-portal or use an X11/XWayland session."
        ),
        ScreenCastPortalProbe::MissingDbusTools => format!(
            "Wayland session {wayland_display} was detected without XWayland. The app could not inspect ScreenCast portal readiness because neither gdbus nor busctl is available."
        ),
        ScreenCastPortalProbe::Unreachable => format!(
            "Wayland session {wayland_display} was detected without XWayland. A ScreenCast portal may be installed, but it could not be reached on the session bus."
        ),
    };

    CaptureError::BackendUnavailable(guidance)
}

fn restore_token_cache() -> &'static Mutex<Option<String>> {
    CURRENT_RESTORE_TOKEN.get_or_init(|| Mutex::new(None))
}

fn cache_restore_token(token: Option<String>) {
    if let Ok(mut slot) = restore_token_cache().lock() {
        *slot = token;
    }
}

pub fn current_restore_token() -> Option<String> {
    restore_token_cache()
        .lock()
        .ok()
        .and_then(|token| token.clone())
}

pub fn probe_screen_cast_portal() -> ScreenCastPortalProbe {
    let source_types = query_portal_u32_property("AvailableSourceTypes");
    let cursor_modes = query_portal_u32_property("AvailableCursorModes");

    match (source_types, cursor_modes) {
        (Some(available_source_types), Some(available_cursor_modes)) => {
            ScreenCastPortalProbe::Available(ScreenCastPortalCapabilities {
                available_source_types,
                available_cursor_modes,
            })
        }
        _ if command_succeeds("xdg-desktop-portal", &["--version"]) => {
            ScreenCastPortalProbe::Unreachable
        }
        _ if command_exists("gdbus") || command_exists("busctl") => {
            ScreenCastPortalProbe::MissingPortal
        }
        _ => ScreenCastPortalProbe::MissingDbusTools,
    }
}

pub fn gstreamer_pipewire_support() -> PipeWireGstreamerSupport {
    if !command_exists("gst-launch-1.0") || !command_exists("gst-inspect-1.0") {
        return PipeWireGstreamerSupport::Unknown;
    }

    let required_plugins = ["pipewiresrc", "x264enc", "mp4mux"];
    if required_plugins
        .iter()
        .all(|plugin| command_succeeds("gst-inspect-1.0", &[*plugin]))
    {
        PipeWireGstreamerSupport::Available
    } else {
        PipeWireGstreamerSupport::Missing
    }
}

#[allow(dead_code)]
pub fn command_plan(
    request_token: &str,
    session_token: &str,
    parent_window: Option<&str>,
    restore_token: Option<&str>,
) -> ScreenCastPortalCommandPlan {
    let session_handle = expected_session_handle(session_token);

    ScreenCastPortalCommandPlan {
        create_session: build_create_session_gdbus_args(request_token, session_token),
        select_sources: build_select_sources_gdbus_args(&session_handle, restore_token),
        start: build_start_gdbus_args(&session_handle, parent_window),
        open_pipewire_remote: build_open_pipewire_remote_gdbus_args(&session_handle),
    }
}

#[allow(dead_code)]
pub fn negotiate_runtime_session(
    parent_window: Option<&str>,
    restore_token: Option<&str>,
) -> Result<ScreenCastPortalRuntimeSession, CaptureError> {
    eprintln!(
        "[wayland-portal] negotiating ScreenCast session with parent_window={} restore_token={}",
        parent_window.unwrap_or("<none>"),
        if restore_token.is_some() {
            "<provided>"
        } else {
            "<none>"
        }
    );
    let connection = Connection::session().map_err(|error| {
        CaptureError::BackendUnavailable(format!(
            "failed to connect to the Wayland session bus: {error}"
        ))
    })?;
    let sender = normalized_unique_name(&connection)?;
    let screen_cast_proxy = Proxy::new(
        &connection,
        SCREENCAST_DESTINATION,
        SCREENCAST_OBJECT_PATH,
        SCREENCAST_INTERFACE,
    )
    .map_err(|error| {
        CaptureError::BackendUnavailable(format!("failed to open ScreenCast portal proxy: {error}"))
    })?;

    let session_token = next_token("rs_session");
    let create_request_token = next_token("rs_create");
    let create_request_path = request_handle_path(&sender, &create_request_token);
    let create_response_waiter =
        spawn_response_waiter(&connection, &create_request_path, "CreateSession")?;

    let create_options = HashMap::from([
        (
            "handle_token",
            OwnedValue::from(Str::from(create_request_token.as_str())),
        ),
        (
            "session_handle_token",
            OwnedValue::from(Str::from(session_token.as_str())),
        ),
    ]);
    let create_request_handle: OwnedObjectPath = screen_cast_proxy
        .call("CreateSession", &create_options)
        .map_err(|error| portal_stage_failed("CreateSession", error))?;
    let create_request_handle = create_request_handle.to_string();
    let create_response_waiter = if create_request_handle != create_request_path {
        eprintln!(
            "[wayland-portal] CreateSession returned request handle `{actual}` instead of predicted `{predicted}`; switching listener",
            actual = create_request_handle,
            predicted = create_request_path
        );
        spawn_response_waiter(&connection, &create_request_handle, "CreateSession")?
    } else {
        create_response_waiter
    };
    let (create_response, create_results) = create_response_waiter.wait()?;
    ensure_portal_success("CreateSession", create_response)?;

    let session_handle = extract_session_handle(&create_results)
        .unwrap_or_else(|| create_session_handle_path(&sender, &session_token));
    let session_restore_token = extract_restore_token(&create_results);

    let select_request_token = next_token("rs_select");
    let select_request_path = request_handle_path(&sender, &select_request_token);
    let select_response_waiter =
        spawn_response_waiter(&connection, &select_request_path, "SelectSources")?;
    let mut select_options = HashMap::from([
        (
            "handle_token",
            OwnedValue::from(Str::from(select_request_token.as_str())),
        ),
        ("multiple", OwnedValue::from(false)),
        ("types", OwnedValue::from(1_u32)),
        ("cursor_mode", OwnedValue::from(2_u32)),
        ("persist_mode", OwnedValue::from(2_u32)),
    ]);
    if let Some(restore_token) = restore_token.filter(|token| !token.trim().is_empty()) {
        select_options.insert("restore_token", OwnedValue::from(Str::from(restore_token)));
    }
    let select_request_handle: OwnedObjectPath = screen_cast_proxy
        .call(
            "SelectSources",
            &(path_from_string(&session_handle)?, select_options),
        )
        .map_err(|error| portal_stage_failed("SelectSources", error))?;
    let select_request_handle = select_request_handle.to_string();
    let select_response_waiter = if select_request_handle != select_request_path {
        eprintln!(
            "[wayland-portal] SelectSources returned request handle `{actual}` instead of predicted `{predicted}`; switching listener",
            actual = select_request_handle,
            predicted = select_request_path
        );
        spawn_response_waiter(&connection, &select_request_handle, "SelectSources")?
    } else {
        select_response_waiter
    };
    let (select_response, _select_results) = select_response_waiter.wait()?;
    ensure_portal_success("SelectSources", select_response)?;

    let start_request_token = next_token("rs_start");
    let start_request_path = request_handle_path(&sender, &start_request_token);
    let start_response_waiter = spawn_response_waiter(&connection, &start_request_path, "Start")?;
    let start_options = HashMap::from([(
        "handle_token",
        OwnedValue::from(Str::from(start_request_token.as_str())),
    )]);
    let parent_window = parent_window.unwrap_or("");
    let start_request_handle: OwnedObjectPath = screen_cast_proxy
        .call(
            "Start",
            &(
                path_from_string(&session_handle)?,
                parent_window,
                start_options,
            ),
        )
        .map_err(|error| portal_stage_failed("Start", error))?;
    let start_request_handle = start_request_handle.to_string();
    let start_response_waiter = if start_request_handle != start_request_path {
        eprintln!(
            "[wayland-portal] Start returned request handle `{actual}` instead of predicted `{predicted}`; switching listener",
            actual = start_request_handle,
            predicted = start_request_path
        );
        spawn_response_waiter(&connection, &start_request_handle, "Start")?
    } else {
        start_response_waiter
    };
    let (start_response, start_results) = start_response_waiter.wait()?;
    ensure_portal_success("Start", start_response)?;

    let final_session_handle = extract_session_handle(&start_results).unwrap_or(session_handle);
    eprintln!("[wayland-portal] Start results: {start_results:?}");
    let stream_targets = extract_stream_targets(&start_results);
    if stream_targets.is_empty() {
        return Err(CaptureError::BackendUnavailable(
            "ScreenCast portal Start succeeded but did not return any PipeWire stream nodes."
                .to_string(),
        ));
    }
    let stream_node_ids = stream_targets.iter().map(|(node_id, _)| *node_id).collect();
    let stream_target_ids = stream_targets
        .into_iter()
        .filter_map(|(_, target_id)| target_id)
        .collect();

    let pipewire_remote_fd: OwnedFd = screen_cast_proxy
        .call(
            "OpenPipeWireRemote",
            &(path_from_string(&final_session_handle)?, empty_options()),
        )
        .map_err(|error| portal_stage_failed("OpenPipeWireRemote", error))?;

    let restore_token = extract_restore_token(&start_results).or(session_restore_token);
    cache_restore_token(restore_token.clone());

    Ok(ScreenCastPortalRuntimeSession {
        session_handle: final_session_handle,
        restore_token,
        stream_node_ids,
        stream_target_ids,
        pipewire_remote_fd,
    })
}

#[allow(dead_code)]
pub fn runtime_pending_copy(runtime_session: &ScreenCastPortalRuntimeSession) -> String {
    let restore_suffix = runtime_session
        .restore_token
        .as_deref()
        .map(|token| format!(" Restore token `{token}` was issued by the portal."))
        .unwrap_or_default();

    format!(
        "The ScreenCast portal session was negotiated successfully and returned PipeWire stream node IDs {:?}. The portal also opened a PipeWire remote file descriptor, but the recorder does not ingest that fd into a live Wayland capture pipeline yet.{}",
        runtime_session.stream_node_ids, restore_suffix
    )
}

fn query_portal_u32_property(property: &str) -> Option<u32> {
    query_portal_property_with_gdbus(property)
        .or_else(|| query_portal_property_with_busctl(property))
}

fn query_portal_property_with_gdbus(property: &str) -> Option<u32> {
    if !command_exists("gdbus") {
        return None;
    }

    let output = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.freedesktop.portal.ScreenCast",
            property,
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_first_u32(&String::from_utf8_lossy(&output.stdout))
}

#[allow(dead_code)]
fn build_create_session_gdbus_args(request_token: &str, session_token: &str) -> Vec<String> {
    vec![
        "call".to_string(),
        "--session".to_string(),
        "--dest".to_string(),
        SCREENCAST_DESTINATION.to_string(),
        "--object-path".to_string(),
        SCREENCAST_OBJECT_PATH.to_string(),
        "--method".to_string(),
        format!("{SCREENCAST_INTERFACE}.CreateSession"),
        format!(
            "{{'handle_token': <'{request_token}'>, 'session_handle_token': <'{session_token}'>}}"
        ),
    ]
}

#[allow(dead_code)]
fn build_select_sources_gdbus_args(
    session_handle: &str,
    restore_token: Option<&str>,
) -> Vec<String> {
    let options = if let Some(restore_token) =
        restore_token.filter(|token| !token.trim().is_empty())
    {
        format!(
            "{{'multiple': <false>, 'types': <uint32 1>, 'cursor_mode': <uint32 2>, 'persist_mode': <uint32 2>, 'restore_token': <'{restore_token}'>}}"
        )
    } else {
        "{'multiple': <false>, 'types': <uint32 1>, 'cursor_mode': <uint32 2>, 'persist_mode': <uint32 2>}".to_string()
    };
    vec![
        "call".to_string(),
        "--session".to_string(),
        "--dest".to_string(),
        SCREENCAST_DESTINATION.to_string(),
        "--object-path".to_string(),
        SCREENCAST_OBJECT_PATH.to_string(),
        "--method".to_string(),
        format!("{SCREENCAST_INTERFACE}.SelectSources"),
        session_handle.to_string(),
        options,
    ]
}

#[allow(dead_code)]
fn build_start_gdbus_args(session_handle: &str, parent_window: Option<&str>) -> Vec<String> {
    vec![
        "call".to_string(),
        "--session".to_string(),
        "--dest".to_string(),
        SCREENCAST_DESTINATION.to_string(),
        "--object-path".to_string(),
        SCREENCAST_OBJECT_PATH.to_string(),
        "--method".to_string(),
        format!("{SCREENCAST_INTERFACE}.Start"),
        session_handle.to_string(),
        parent_window.unwrap_or("").to_string(),
        "{}".to_string(),
    ]
}

#[allow(dead_code)]
fn build_open_pipewire_remote_gdbus_args(session_handle: &str) -> Vec<String> {
    vec![
        "call".to_string(),
        "--session".to_string(),
        "--dest".to_string(),
        SCREENCAST_DESTINATION.to_string(),
        "--object-path".to_string(),
        SCREENCAST_OBJECT_PATH.to_string(),
        "--method".to_string(),
        format!("{SCREENCAST_INTERFACE}.OpenPipeWireRemote"),
        session_handle.to_string(),
        "{}".to_string(),
    ]
}

#[allow(dead_code)]
fn expected_session_handle(session_token: &str) -> String {
    format!("/org/freedesktop/portal/desktop/session/unknown/{session_token}")
}

fn query_portal_property_with_busctl(property: &str) -> Option<u32> {
    if !command_exists("busctl") {
        return None;
    }

    let output = Command::new("busctl")
        .args([
            "--user",
            "get-property",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.ScreenCast",
            property,
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_first_u32(&String::from_utf8_lossy(&output.stdout))
}

fn normalized_unique_name(connection: &Connection) -> Result<String, CaptureError> {
    connection
        .unique_name()
        .map(|name| name.as_str().trim_start_matches(':').replace('.', "_"))
        .ok_or_else(|| {
            CaptureError::BackendUnavailable(
                "the Wayland session bus did not expose a unique name for ScreenCast negotiation"
                    .to_string(),
            )
        })
}

fn next_token(prefix: &str) -> String {
    static TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let ordinal = TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{millis}_{ordinal}")
}

fn request_handle_path(sender: &str, request_token: &str) -> String {
    format!("/org/freedesktop/portal/desktop/request/{sender}/{request_token}")
}

fn create_session_handle_path(sender: &str, session_token: &str) -> String {
    format!("/org/freedesktop/portal/desktop/session/{sender}/{session_token}")
}

fn request_proxy<'a>(
    connection: &'a Connection,
    request_path: &'a str,
) -> Result<Proxy<'a>, CaptureError> {
    Proxy::new(
        connection,
        SCREENCAST_DESTINATION,
        request_path,
        "org.freedesktop.portal.Request",
    )
    .map_err(|error| {
        CaptureError::BackendUnavailable(format!(
            "failed to open ScreenCast request proxy for `{request_path}`: {error}"
        ))
    })
}

type PortalResponse = (u32, HashMap<String, OwnedValue>);

struct ResponseWaiter {
    stage: &'static str,
    response_rx: mpsc::Receiver<Result<PortalResponse, String>>,
}

impl ResponseWaiter {
    fn wait(self) -> Result<PortalResponse, CaptureError> {
        eprintln!(
            "[wayland-portal] waiting for {stage} response (timeout: {}s)",
            PORTAL_RESPONSE_TIMEOUT.as_secs(),
            stage = self.stage
        );

        match self.response_rx.recv_timeout(PORTAL_RESPONSE_TIMEOUT) {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(error)) => Err(CaptureError::BackendUnavailable(format!(
                "ScreenCast portal {stage} response failed: {error}",
                stage = self.stage
            ))),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let journal_hint = recent_portal_failure_hint()
                    .map(|hint| format!(" Recent portal logs: {hint}"))
                    .unwrap_or_default();
                Err(CaptureError::BackendUnavailable(format!(
                    "ScreenCast portal {stage} timed out after {}s. The desktop picker may be waiting for approval, or the portal may have stalled.{journal_hint}",
                    PORTAL_RESPONSE_TIMEOUT.as_secs(),
                    stage = self.stage
                )))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(CaptureError::BackendUnavailable(format!(
                    "ScreenCast portal {stage} response listener disconnected before returning a result.",
                    stage = self.stage
                )))
            }
        }
    }
}

fn spawn_response_waiter(
    connection: &Connection,
    request_path: &str,
    stage: &'static str,
) -> Result<ResponseWaiter, CaptureError> {
    let connection = connection.clone();
    let request_path = request_path.to_string();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let (response_tx, response_rx) = mpsc::sync_channel(1);

    thread::spawn(move || {
        let setup_and_wait = (|| -> Result<PortalResponse, String> {
            let proxy = request_proxy(&connection, &request_path).map_err(|error| match error {
                CaptureError::BackendUnavailable(message) => message,
                other => other.to_string(),
            })?;
            let mut response_stream = proxy
                .receive_signal("Response")
                .map_err(|error| error.to_string())?;
            let _ = ready_tx.send(Ok(()));

            let message = response_stream.next().ok_or_else(|| {
                "the ScreenCast portal closed before returning a response".to_string()
            })?;

            message
                .body()
                .deserialize::<PortalResponse>()
                .map_err(|error| error.to_string())
        })();

        if let Err(error) = &setup_and_wait {
            let _ = ready_tx.try_send(Err(error.clone()));
        }

        let _ = response_tx.send(setup_and_wait);
    });

    match ready_rx.recv_timeout(PORTAL_LISTENER_READY_TIMEOUT) {
        Ok(Ok(())) => {
            eprintln!(
                "[wayland-portal] listening for {stage} response",
                stage = stage
            );
            Ok(ResponseWaiter { stage, response_rx })
        }
        Ok(Err(error)) => Err(CaptureError::BackendUnavailable(format!(
            "failed to subscribe to ScreenCast portal {stage} response: {error}",
            stage = stage
        ))),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(CaptureError::BackendUnavailable(format!(
            "timed out after {}s while preparing to listen for ScreenCast portal {stage} response",
            PORTAL_LISTENER_READY_TIMEOUT.as_secs(),
            stage = stage
        ))),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(CaptureError::BackendUnavailable(format!(
                "the ScreenCast portal {stage} response listener disconnected before subscribing.",
                stage = stage
            )))
        }
    }
}

fn portal_stage_failed(stage: &str, error: zbus::Error) -> CaptureError {
    CaptureError::BackendUnavailable(format!("ScreenCast portal {stage} call failed: {error}"))
}

fn recent_portal_failure_hint() -> Option<String> {
    if !command_exists("journalctl") {
        return None;
    }

    let output = Command::new("journalctl")
        .args([
            "--user",
            "--since",
            "2 minutes ago",
            "--no-pager",
            "-u",
            "xdg-desktop-portal.service",
            "-u",
            "xdg-desktop-portal-gnome.service",
            "-u",
            "xdg-desktop-portal-gtk.service",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let logs = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut hints = Vec::new();

    if logs.contains("Failed to associate portal window with parent window") {
        hints.push("failed to associate the portal dialog with a parent window");
    }
    if logs.contains("status=11/SEGV") || logs.contains("Failed with result 'core-dump'") {
        hints.push("xdg-desktop-portal-gnome crashed");
    }
    if logs.contains("NoReply") {
        hints.push("the portal implementation disconnected without replying");
    }

    if hints.is_empty() {
        None
    } else {
        Some(join_items(&hints))
    }
}

fn ensure_portal_success(stage: &str, response: u32) -> Result<(), CaptureError> {
    if response == 0 {
        return Ok(());
    }

    let explanation = match response {
        1 => "the portal denied the request or the user dismissed the picker",
        2 => "the portal cancelled the request",
        _ => "the portal returned a non-success response",
    };

    Err(CaptureError::BackendUnavailable(format!(
        "ScreenCast portal {stage} failed with response code {response}: {explanation}"
    )))
}

fn path_from_string(path: &str) -> Result<OwnedObjectPath, CaptureError> {
    OwnedObjectPath::try_from(path).map_err(|error| {
        CaptureError::BackendUnavailable(format!(
            "ScreenCast portal returned an invalid object path `{path}`: {error}"
        ))
    })
}

fn empty_options() -> HashMap<&'static str, OwnedValue> {
    HashMap::new()
}

fn extract_session_handle(results: &HashMap<String, OwnedValue>) -> Option<String> {
    results
        .get("session_handle")
        .and_then(|value| owned_value_to_string(value))
}

fn extract_restore_token(results: &HashMap<String, OwnedValue>) -> Option<String> {
    results
        .get("restore_token")
        .and_then(|value| owned_value_to_string(value))
}

fn extract_stream_node_ids(results: &HashMap<String, OwnedValue>) -> Vec<u32> {
    extract_stream_targets(results)
        .into_iter()
        .map(|(node_id, _)| node_id)
        .collect()
}

fn extract_stream_targets(results: &HashMap<String, OwnedValue>) -> Vec<(u32, Option<String>)> {
    let Some(streams) = results.get("streams") else {
        return Vec::new();
    };

    if let Ok(entries) = <Vec<(u32, HashMap<String, OwnedValue>)>>::try_from(streams.clone()) {
        return entries
            .into_iter()
            .map(|(node_id, properties)| {
                let target_id = properties
                    .get("id")
                    .and_then(owned_value_to_string)
                    .filter(|value| !value.trim().is_empty());
                (node_id, target_id)
            })
            .collect();
    }

    let debug = format!("{streams:?}");
    parse_stream_node_ids_from_response(&debug)
        .into_iter()
        .map(|node_id| (node_id, None))
        .collect()
}

fn owned_value_to_string(value: &OwnedValue) -> Option<String> {
    if let Ok(path) = <OwnedObjectPath as TryFrom<OwnedValue>>::try_from(value.clone()) {
        return Some(path.to_string());
    }

    <String as TryFrom<OwnedValue>>::try_from(value.clone()).ok()
}

fn command_exists(program: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {program} >/dev/null 2>&1")])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn command_succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn pipewire_support_copy(support: PipeWireGstreamerSupport) -> &'static str {
    match support {
        PipeWireGstreamerSupport::Available => {
            "The required GStreamer PipeWire plugins are installed, so the app can route pure Wayland capture through the native ScreenCast portal + PipeWire + GStreamer path."
        }
        PipeWireGstreamerSupport::Missing => {
            "The ScreenCast portal is reachable, but the required GStreamer PipeWire plugins are missing. Install `pipewiresrc`, `x264enc`, and `mp4mux` support before pure Wayland recording can start."
        }
        PipeWireGstreamerSupport::Unknown => {
            "The app could negotiate the ScreenCast portal lifecycle, but the required GStreamer runtime could not be confirmed on this machine."
        }
    }
}

fn parse_first_u32(output: &str) -> Option<u32> {
    output
        .split(|character: char| !character.is_ascii_digit())
        .filter(|token| !token.is_empty())
        .last()
        .and_then(|token| token.parse::<u32>().ok())
}

#[allow(dead_code)]
pub fn parse_request_handle(output: &str) -> Option<String> {
    parse_first_quoted_path(output)
}

#[allow(dead_code)]
pub fn parse_session_handle_from_response(output: &str) -> Option<String> {
    extract_variant_string(output, "session_handle")
}

#[allow(dead_code)]
pub fn parse_restore_token_from_response(output: &str) -> Option<String> {
    extract_variant_string(output, "restore_token")
}

#[allow(dead_code)]
pub fn parse_stream_node_ids_from_response(output: &str) -> Vec<u32> {
    let Some(streams_section) = output.split("string \"streams\"").nth(1) else {
        return Vec::new();
    };

    streams_section
        .split("struct {")
        .skip(1)
        .filter_map(|chunk| {
            chunk
                .split("uint32")
                .nth(1)
                .map(str::trim)
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse::<u32>().ok())
        })
        .collect()
}

#[allow(dead_code)]
pub fn parse_start_response(output: &str) -> Option<ScreenCastPortalStartResult> {
    let session_handle = parse_session_handle_from_response(output)?;
    let restore_token = parse_restore_token_from_response(output);
    let stream_node_ids = parse_stream_node_ids_from_response(output);

    Some(ScreenCastPortalStartResult {
        session_handle,
        restore_token,
        stream_node_ids,
    })
}

fn parse_first_quoted_path(output: &str) -> Option<String> {
    let start = output.find('\'')? + 1;
    let end = output[start..].find('\'')? + start;
    Some(output[start..end].to_string())
}

fn extract_variant_string(output: &str, key: &str) -> Option<String> {
    let key_index = output.find(key)?;
    let tail = &output[key_index + key.len()..];

    if let Some(marker_index) = tail.find("objectpath '") {
        let remainder = &tail[marker_index + "objectpath '".len()..];
        if let Some(end) = remainder.find('\'') {
            return Some(remainder[..end].to_string());
        }
    }

    if let Some(marker_index) = tail.find("string \"") {
        let remainder = &tail[marker_index + "string \"".len()..];
        if let Some(end) = remainder.find('"') {
            return Some(remainder[..end].to_string());
        }
    }

    None
}

fn describe_source_types(mask: u32) -> String {
    let mut items = Vec::new();

    if mask & 1 != 0 {
        items.push("monitor sharing");
    }
    if mask & 2 != 0 {
        items.push("window sharing");
    }
    if mask & 4 != 0 {
        items.push("virtual displays");
    }

    if items.is_empty() {
        "no source types".to_string()
    } else {
        join_items(&items)
    }
}

fn describe_cursor_modes(mask: u32) -> String {
    let mut items = Vec::new();

    if mask & 1 != 0 {
        items.push("hidden cursor");
    }
    if mask & 2 != 0 {
        items.push("embedded cursor");
    }
    if mask & 4 != 0 {
        items.push("cursor metadata");
    }

    if items.is_empty() {
        "no cursor modes".to_string()
    } else {
        join_items(&items)
    }
}

fn join_items(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [single] => (*single).to_string(),
        [head @ .., tail] => format!("{} and {}", head.join(", "), tail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PipeWireGstreamerSupport, ScreenCastPortalProbe, command_plan, describe_cursor_modes,
        describe_source_types, join_items, parse_first_u32, parse_request_handle,
        parse_restore_token_from_response, parse_session_handle_from_response,
        parse_start_response, parse_stream_node_ids_from_response, pipewire_support_copy,
    };

    #[test]
    fn parses_first_integer_from_gdbus_output() {
        assert_eq!(parse_first_u32("(<'u', uint32 3>,)").as_ref(), Some(&3_u32));
        assert_eq!(parse_first_u32("(uint32 7,)").as_ref(), Some(&7_u32));
    }

    #[test]
    fn describes_portal_source_types() {
        assert_eq!(describe_source_types(1), "monitor sharing");
        assert_eq!(
            describe_source_types(3),
            "monitor sharing and window sharing"
        );
    }

    #[test]
    fn describes_cursor_modes() {
        assert_eq!(describe_cursor_modes(2), "embedded cursor");
        assert_eq!(
            describe_cursor_modes(6),
            "embedded cursor and cursor metadata"
        );
    }

    #[test]
    fn joins_items_like_human_copy() {
        assert_eq!(join_items(&["a"]), "a");
        assert_eq!(join_items(&["a", "b"]), "a and b");
        assert_eq!(join_items(&["a", "b", "c"]), "a, b and c");
    }

    #[test]
    fn probe_enum_is_constructible() {
        let probe = ScreenCastPortalProbe::MissingPortal;
        assert!(matches!(probe, ScreenCastPortalProbe::MissingPortal));
    }

    #[test]
    fn renders_pipewire_support_copy() {
        assert!(pipewire_support_copy(PipeWireGstreamerSupport::Available).contains("GStreamer"));
        assert!(pipewire_support_copy(PipeWireGstreamerSupport::Missing).contains("plugins"));
        assert!(pipewire_support_copy(PipeWireGstreamerSupport::Unknown).contains("runtime"));
    }

    #[test]
    fn parses_request_handle_from_gdbus_call() {
        assert_eq!(
            parse_request_handle("('/org/freedesktop/portal/desktop/request/1_42/rs_req',)"),
            Some("/org/freedesktop/portal/desktop/request/1_42/rs_req".to_string())
        );
    }

    #[test]
    fn parses_session_and_restore_token_from_response() {
        let response = "signal time=1 sender=:1.20 -> destination=:1.200 serial=55 path=/org/freedesktop/portal/desktop/request/1_20/rs_req; interface=org.freedesktop.portal.Request; member=Response\n   uint32 0\n   array [\n      dict entry(\n         string \"session_handle\"\n         variant             objectpath '/org/freedesktop/portal/desktop/session/1_20/rs_session'\n      )\n      dict entry(\n         string \"restore_token\"\n         variant             string \"restore-123\"\n      )\n   ]";

        assert_eq!(
            parse_session_handle_from_response(response),
            Some("/org/freedesktop/portal/desktop/session/1_20/rs_session".to_string())
        );
        assert_eq!(
            parse_restore_token_from_response(response),
            Some("restore-123".to_string())
        );
    }

    #[test]
    fn parses_stream_node_ids_from_start_response() {
        let response = "signal time=1 sender=:1.20 -> destination=:1.200 serial=77 path=/org/freedesktop/portal/desktop/request/1_20/rs_start; interface=org.freedesktop.portal.Request; member=Response\n   uint32 0\n   array [\n      dict entry(\n         string \"session_handle\"\n         variant             objectpath '/org/freedesktop/portal/desktop/session/1_20/rs_session'\n      )\n      dict entry(\n         string \"streams\"\n         variant             array [\n               struct {\n                  uint32 58\n                  array [\n                     dict entry(\n                        string \"source_type\"\n                        variant                            uint32 1\n                     )\n                  ]\n               }\n               struct {\n                  uint32 73\n                  array [\n                     dict entry(\n                        string \"source_type\"\n                        variant                            uint32 2\n                     )\n                  ]\n               }\n            ]\n      )\n   ]";

        assert_eq!(parse_stream_node_ids_from_response(response), vec![58, 73]);

        let start = parse_start_response(response).expect("expected parsed start response");
        assert_eq!(
            start.session_handle,
            "/org/freedesktop/portal/desktop/session/1_20/rs_session"
        );
        assert_eq!(start.stream_node_ids, vec![58, 73]);
    }

    #[test]
    fn builds_gdbus_command_plan_for_lifecycle() {
        let plan = command_plan(
            "request-1",
            "session-1",
            Some("wayland:record-screen-test-parent"),
            Some("restore-token-1"),
        );

        assert!(
            plan.create_session
                .iter()
                .any(|item| item.contains("CreateSession"))
        );
        assert!(
            plan.select_sources
                .iter()
                .any(|item| item.contains("SelectSources"))
        );
        assert!(
            plan.select_sources
                .iter()
                .any(|item| item.contains("restore-token-1"))
        );
        assert!(plan.start.iter().any(|item| item.contains("Start")));
        assert!(
            plan.start
                .iter()
                .any(|item| item.contains("wayland:record-screen-test-parent"))
        );
        assert!(
            plan.open_pipewire_remote
                .iter()
                .any(|item| item.contains("OpenPipeWireRemote"))
        );
    }
}
