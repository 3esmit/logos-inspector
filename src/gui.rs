use std::{
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result};
use eframe::egui;
use lee::AccountId;
use logos_inspector::{
    CUSTOM_NETWORK_PROFILE, DEFAULT_INDEXER_ENDPOINT, DEFAULT_NETWORK_PROFILE,
    DEFAULT_SEQUENCER_ENDPOINT, account_lookup, account_lookup_with_idl,
    decode_account_data_hex_with_idl, decode_event_data_hex_with_idl,
    decode_instruction_words_with_idl, last_sequencer_block_id, network_profiles, overview,
    program_file_info, raw_rpc_report, resolve_network_endpoints, sequencer_block,
    sequencer_program_ids, sequencer_transaction, sequencer_transaction_inspection,
    sequencer_transaction_inspection_with_idl, sequencer_transaction_trace,
    sequencer_transaction_trace_with_idl,
};
use serde_json::Value;

type TaskResult = Result<Value, String>;

const IDL_STORAGE_KEY: &str = "logos_inspector_idl_definitions_v1";

const BG: egui::Color32 = egui::Color32::from_rgb(21, 21, 21);
const SIDE_BG: egui::Color32 = egui::Color32::from_rgb(18, 18, 17);
const CARD: egui::Color32 = egui::Color32::from_rgb(30, 30, 28);
const INPUT: egui::Color32 = egui::Color32::from_rgb(16, 16, 16);
const PANEL: egui::Color32 = egui::Color32::from_rgb(42, 41, 38);
const PANEL_HOVER: egui::Color32 = egui::Color32::from_rgb(56, 53, 49);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(242, 106, 33);
const ACCENT_DARK: egui::Color32 = egui::Color32::from_rgb(50, 30, 20);
const GREEN: egui::Color32 = egui::Color32::from_rgb(88, 184, 126);
const TEXT: egui::Color32 = egui::Color32::from_rgb(231, 225, 216);
const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(191, 183, 174);
const BORDER: egui::Color32 = egui::Color32::from_rgb(108, 101, 91);
const BORDER_STRONG: egui::Color32 = egui::Color32::from_rgb(146, 132, 112);
const ACCENT_TEXT: egui::Color32 = egui::Color32::from_rgb(33, 22, 15);
const WINDOW_EDGE: egui::Color32 = egui::Color32::from_rgb(86, 80, 72);
const CLOSE_IDLE: egui::Color32 = egui::Color32::from_rgb(54, 35, 31);
const SIDEBAR_WIDTH: f32 = 208.0;
const MAIN_MAX_WIDTH: f32 = 1280.0;
const ROOT_STACK_WIDTH: f32 = 900.0;
const DASHBOARD_TWO_COLUMN_MIN_WIDTH: f32 = 1240.0;
const DASHBOARD_COMPACT_ROW_WIDTH: f32 = 700.0;
const DASHBOARD_BLOCK_LIMIT: usize = 2;
const DASHBOARD_TRANSACTION_LIMIT: usize = 4;
const CUSTOM_CHROME_HEIGHT: f32 = 48.0;
const CUSTOM_CHROME_CONTENT_INSET: f32 = 8.0;
const RESIZE_EDGE_THICKNESS: f32 = 6.0;
const RESIZE_CORNER_SIZE: f32 = 16.0;
const MIN_WINDOW_WIDTH: f32 = 640.0;
const MIN_WINDOW_HEIGHT: f32 = 560.0;
const OVERVIEW_REFRESH_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Overview,
    Sequencer,
    Accounts,
    Programs,
    Indexer,
    Network,
}

impl View {
    const ALL: [(Self, &'static str); 6] = [
        (Self::Overview, "Dashboard"),
        (Self::Sequencer, "Sequencer"),
        (Self::Accounts, "Accounts"),
        (Self::Programs, "Programs"),
        (Self::Indexer, "Indexer"),
        (Self::Network, "Settings"),
    ];

    const fn title(self) -> &'static str {
        match self {
            Self::Overview => "Dashboard",
            Self::Sequencer => "Sequencer",
            Self::Accounts => "Accounts",
            Self::Programs => "Programs",
            Self::Indexer => "Indexer",
            Self::Network => "Settings",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequencerTab {
    Blocks,
    Transactions,
}

impl SequencerTab {
    const ALL: [(Self, &'static str); 2] = [
        (Self::Blocks, "Blocks"),
        (Self::Transactions, "Transactions"),
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndexerTab {
    Status,
    Rpc,
}

impl IndexerTab {
    const ALL: [(Self, &'static str); 2] = [(Self::Status, "Status"), (Self::Rpc, "RPC")];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgramsTab {
    Binaries,
    Idls,
    Events,
}

impl ProgramsTab {
    const ALL: [(Self, &'static str); 3] = [
        (Self::Binaries, "Binaries"),
        (Self::Idls, "IDLs"),
        (Self::Events, "Events"),
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionTab {
    Summary,
    Structure,
    Trace,
}

impl TransactionTab {
    const ALL: [(Self, &'static str); 3] = [
        (Self::Summary, "Summary"),
        (Self::Structure, "Structure"),
        (Self::Trace, "Trace"),
    ];

    const fn action_label(self) -> &'static str {
        match self {
            Self::Summary => "Inspect transaction",
            Self::Structure => "Inspect structure",
            Self::Trace => "Trace execution",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccountTab {
    Lookup,
    DecodeData,
}

impl AccountTab {
    const ALL: [(Self, &'static str); 2] =
        [(Self::Lookup, "Lookup"), (Self::DecodeData, "Decode data")];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccountOutputTab {
    Decoded,
    Detail,
    Raw,
}

impl AccountOutputTab {
    const ALL: [(Self, &'static str); 3] = [
        (Self::Decoded, "Decoded Output"),
        (Self::Detail, "Detail Output"),
        (Self::Raw, "Raw Output"),
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultScope {
    Overview,
    Sequencer(SequencerTab),
    Accounts(AccountTab),
    Programs(ProgramsTab),
    Indexer(IndexerTab),
    Network,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LookupTarget {
    Account(String),
    Transaction(String),
    Block(u64),
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct RegisteredIdl {
    name: String,
    program_id: Option<String>,
    json: String,
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
struct PersistedIdlState {
    registered_idls: Vec<RegisteredIdl>,
    active_idl_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct DashboardReport {
    overview: logos_inspector::OverviewReport,
    recent_transaction_count: usize,
    recent_tps: Option<f64>,
    recent_window_seconds: Option<u64>,
    latest_blocks: Vec<DashboardBlock>,
    latest_transactions: Vec<DashboardTransaction>,
    block_errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct DashboardBlock {
    block_id: u64,
    timestamp: u64,
    bedrock_status: String,
    tx_count: usize,
    decode_warning: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct DashboardTransaction {
    block_id: u64,
    hash: String,
    kind: String,
    program_id_hex: Option<String>,
    account_count: usize,
    instruction_words: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeAxis {
    East,
    South,
    SouthEast,
}

impl ResizeAxis {
    const fn viewport_direction(self) -> egui::viewport::ResizeDirection {
        match self {
            Self::East => egui::viewport::ResizeDirection::East,
            Self::South => egui::viewport::ResizeDirection::South,
            Self::SouthEast => egui::viewport::ResizeDirection::SouthEast,
        }
    }
}

pub fn run() -> eframe::Result {
    let use_custom_chrome = use_custom_chrome_runtime();
    let has_native_frame = !use_custom_chrome;
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("logos-inspector")
            .with_inner_size([1180.0, 820.0])
            .with_min_inner_size([MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT])
            .with_decorations(has_native_frame)
            .with_resizable(true)
            .with_transparent(false),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    configure_event_loop(&mut options);

    eframe::run_native(
        "Logos Inspector",
        options,
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx);
            Ok(Box::new(LogosInspectorApp::new(cc)))
        }),
    )
}

fn should_prefer_x11_on_wslg() -> bool {
    if std::env::var_os("WSL_INTEROP").is_none()
        || std::env::var_os("DISPLAY").is_none()
        || std::env::var_os("LOGOS_INSPECTOR_ALLOW_WAYLAND").is_some()
    {
        return false;
    }

    true
}

fn use_custom_chrome_runtime() -> bool {
    is_wslg_runtime() && !should_prefer_x11_on_wslg()
}

#[cfg(target_os = "linux")]
fn configure_event_loop(options: &mut eframe::NativeOptions) {
    if should_prefer_x11_on_wslg() {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::x11::EventLoopBuilderExtX11 as _;
            builder.with_x11();
        }));
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_event_loop(_options: &mut eframe::NativeOptions) {}

fn is_wslg_runtime() -> bool {
    std::env::var_os("WSL_INTEROP").is_some() && std::env::var_os("WAYLAND_DISPLAY").is_some()
}

fn strip_wslg_host_resize_frame() {
    let script = r#"
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class LogosWinStyle {
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
  [DllImport("user32.dll", EntryPoint="GetWindowLongPtrW")] public static extern IntPtr GetWindowLongPtr(IntPtr hWnd, int nIndex);
  [DllImport("user32.dll", EntryPoint="SetWindowLongPtrW")] public static extern IntPtr SetWindowLongPtr(IntPtr hWnd, int nIndex, IntPtr dwNewLong);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
}
"@
$GWL_STYLE = -16
$WS_THICKFRAME = 0x00040000
$WS_MAXIMIZEBOX = 0x00010000
$SWP_NOMOVE = 0x0002
$SWP_NOSIZE = 0x0001
$SWP_NOZORDER = 0x0004
$SWP_NOACTIVATE = 0x0010
$SWP_FRAMECHANGED = 0x0020
for ($attempt = 0; $attempt -lt 40; $attempt++) {
  $script:seen = $false
  [LogosWinStyle]::EnumWindows({
    param($hWnd, $lParam)
    if (-not [LogosWinStyle]::IsWindowVisible($hWnd)) {
      return $true
    }
    $text = New-Object System.Text.StringBuilder 256
    [void][LogosWinStyle]::GetWindowText($hWnd, $text, $text.Capacity)
    if (-not $text.ToString().StartsWith("Logos Inspector")) {
      return $true
    }
    $processId = 0
    [void][LogosWinStyle]::GetWindowThreadProcessId($hWnd, [ref]$processId)
    $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
    if (-not $process -or $process.ProcessName -ne "msrdc") {
      return $true
    }
    $style = [LogosWinStyle]::GetWindowLongPtr($hWnd, $GWL_STYLE).ToInt64()
    $next = $style -band (-bnot ($WS_THICKFRAME -bor $WS_MAXIMIZEBOX))
    if ($next -ne $style) {
      [void][LogosWinStyle]::SetWindowLongPtr($hWnd, $GWL_STYLE, [IntPtr]$next)
      [void][LogosWinStyle]::SetWindowPos($hWnd, [IntPtr]::Zero, 0, 0, 0, 0, $SWP_NOMOVE -bor $SWP_NOSIZE -bor $SWP_NOZORDER -bor $SWP_NOACTIVATE -bor $SWP_FRAMECHANGED)
    }
    $script:seen = $true
    return $false
  }, [IntPtr]::Zero) | Out-Null
  if ($script:seen) {
    Start-Sleep -Milliseconds 250
  } else {
    Start-Sleep -Milliseconds 150
  }
}
"#;

    let status = Command::new("/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if status.is_err() {
        match Command::new("powershell.exe")
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg(script)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(_) | Err(_) => {}
        }
    }
}

fn consume_command_or_ctrl(input: &mut egui::InputState, key: egui::Key) -> bool {
    input.consume_key(egui::Modifiers::COMMAND, key)
        || input.consume_key(egui::Modifiers::CTRL, key)
}

fn consume_view_shortcut(input: &mut egui::InputState, key: egui::Key) -> bool {
    consume_command_or_ctrl(input, key) || input.consume_key(egui::Modifiers::ALT, key)
}

struct LogosInspectorApp {
    view: View,
    sequencer_tab: SequencerTab,
    indexer_tab: IndexerTab,
    programs_tab: ProgramsTab,
    transaction_tab: TransactionTab,
    account_tab: AccountTab,
    account_output_tab: AccountOutputTab,
    network_profile: String,
    sequencer_url: String,
    indexer_url: String,
    block_id: String,
    tx_hash: String,
    transaction_idl_json: String,
    account_id: String,
    account_idl_json: String,
    account_idl_type: String,
    account_idl_override_open: bool,
    account_data_hex: String,
    instruction_idl_json: String,
    instruction_program_id: String,
    instruction_words: String,
    instruction_accounts: String,
    event_name: String,
    event_data_hex: String,
    program_idl_program: String,
    program_path: String,
    program_idl_name: String,
    program_idl_json: String,
    program_idl_error: Option<String>,
    registered_idls: Vec<RegisteredIdl>,
    active_idl_name: Option<String>,
    program_ids: Vec<Value>,
    program_ids_error: Option<String>,
    draft_network_profile: String,
    draft_sequencer_url: String,
    draft_indexer_url: String,
    network_config_error: Option<String>,
    indexer_method: String,
    indexer_params: String,
    dashboard_search: String,
    overview_output: Option<Value>,
    overview_error: Option<String>,
    output: Option<Value>,
    output_error: Option<String>,
    result_label: Option<String>,
    result_scope: Option<ResultScope>,
    output_revision: u64,
    pending: Option<String>,
    receiver: Option<mpsc::Receiver<TaskResult>>,
    overview_receiver: Option<mpsc::Receiver<TaskResult>>,
    wslg_style_fix_started: bool,
    last_overview_refresh: Option<Instant>,
    last_overview_success: Option<Instant>,
    scroll_result_into_view: bool,
}

impl Default for LogosInspectorApp {
    fn default() -> Self {
        Self {
            view: View::Overview,
            sequencer_tab: SequencerTab::Blocks,
            indexer_tab: IndexerTab::Status,
            programs_tab: ProgramsTab::Idls,
            transaction_tab: TransactionTab::Structure,
            account_tab: AccountTab::Lookup,
            account_output_tab: AccountOutputTab::Detail,
            network_profile: DEFAULT_NETWORK_PROFILE.to_owned(),
            sequencer_url: DEFAULT_SEQUENCER_ENDPOINT.to_owned(),
            indexer_url: DEFAULT_INDEXER_ENDPOINT.to_owned(),
            block_id: String::new(),
            tx_hash: String::new(),
            transaction_idl_json: String::new(),
            account_id: String::new(),
            account_idl_json: String::new(),
            account_idl_type: String::new(),
            account_idl_override_open: false,
            account_data_hex: String::new(),
            instruction_idl_json: String::new(),
            instruction_program_id: String::new(),
            instruction_words: String::new(),
            instruction_accounts: String::new(),
            event_name: String::new(),
            event_data_hex: String::new(),
            program_idl_program: String::new(),
            program_path: String::new(),
            program_idl_name: String::new(),
            program_idl_json: String::new(),
            program_idl_error: None,
            registered_idls: Vec::new(),
            active_idl_name: None,
            program_ids: Vec::new(),
            program_ids_error: None,
            draft_network_profile: DEFAULT_NETWORK_PROFILE.to_owned(),
            draft_sequencer_url: DEFAULT_SEQUENCER_ENDPOINT.to_owned(),
            draft_indexer_url: DEFAULT_INDEXER_ENDPOINT.to_owned(),
            network_config_error: None,
            indexer_method: "getLastFinalizedBlockId".to_owned(),
            indexer_params: "[]".to_owned(),
            dashboard_search: String::new(),
            overview_output: None,
            overview_error: None,
            output: None,
            output_error: None,
            result_label: None,
            result_scope: None,
            output_revision: 0,
            pending: None,
            receiver: None,
            overview_receiver: None,
            wslg_style_fix_started: false,
            last_overview_refresh: None,
            last_overview_success: None,
            scroll_result_into_view: false,
        }
    }
}

impl LogosInspectorApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();
        if let Some(storage) = cc.storage
            && let Some(saved) = eframe::get_value::<PersistedIdlState>(storage, IDL_STORAGE_KEY)
        {
            app.registered_idls = saved.registered_idls;
            if let Some(active_name) = saved.active_idl_name
                && let Some(idl) = app
                    .registered_idls
                    .iter()
                    .find(|idl| idl.name == active_name)
                    .cloned()
            {
                app.set_active_idl(idl.name, idl.json);
            }
        }
        app
    }

    fn persisted_idl_state(&self) -> PersistedIdlState {
        PersistedIdlState {
            registered_idls: self.registered_idls.clone(),
            active_idl_name: self.active_idl_name.clone(),
        }
    }
}

impl eframe::App for LogosInspectorApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        BG.to_normalized_gamma_f32()
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, IDL_STORAGE_KEY, &self.persisted_idl_state());
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.receive_task();
        self.receive_overview_task();
        apply_theme(ui.ctx());
        self.handle_shortcuts(ui.ctx());
        self.maybe_refresh_overview(ui.ctx());

        let use_custom_chrome = use_custom_chrome_runtime();
        if use_custom_chrome && !self.wslg_style_fix_started {
            self.wslg_style_fix_started = true;
            thread::spawn(strip_wslg_host_resize_frame);
        }
        let window_rect = ui.max_rect();
        ui.painter().rect_filled(window_rect, 0, BG);
        if use_custom_chrome {
            ui.painter().rect_stroke(
                window_rect.shrink(0.5),
                0,
                egui::Stroke::new(1.0, WINDOW_EDGE),
                egui::StrokeKind::Inside,
            );
        }
        egui::Frame::new()
            .fill(BG)
            .inner_margin(egui::Margin::same(0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    if use_custom_chrome {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), CUSTOM_CHROME_HEIGHT),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| self.window_chrome(ui),
                        );
                        divider(ui);
                    }

                    if use_custom_chrome {
                        egui::Frame::new()
                            .inner_margin(egui::Margin {
                                left: 0,
                                right: CUSTOM_CHROME_CONTENT_INSET as i8,
                                top: 0,
                                bottom: CUSTOM_CHROME_CONTENT_INSET as i8,
                            })
                            .show(ui, |ui| self.app_body(ui));
                    } else {
                        self.app_body(ui);
                    }
                });
            });
        self.account_idl_override_modal(ui.ctx());
        if use_custom_chrome {
            self.resize_handles(ui, window_rect);
            paint_resize_grip(ui, window_rect);
        }
    }
}

impl LogosInspectorApp {
    fn app_body(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        if available.x < ROOT_STACK_WIDTH {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                self.compact_sidebar(ui);
                divider(ui);
                ui.allocate_ui_with_layout(
                    egui::vec2(available.x, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.main_content(ui),
                );
            });
        } else {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(SIDEBAR_WIDTH, available.y),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.sidebar(ui),
                );
                vertical_rule(ui, available.y);
                ui.allocate_ui_with_layout(
                    egui::vec2((available.x - SIDEBAR_WIDTH - 1.0).max(360.0), available.y),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.main_content(ui),
                );
            });
        }
    }

    fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    fn apply_network_profile_to_draft(&mut self) {
        let Ok(endpoints) =
            resolve_network_endpoints(Some(&self.draft_network_profile), None, None)
        else {
            self.draft_network_profile = CUSTOM_NETWORK_PROFILE.to_owned();
            return;
        };
        self.draft_sequencer_url = endpoints.sequencer_endpoint;
        self.draft_indexer_url = endpoints.indexer_endpoint;
        self.network_config_error = None;
    }

    fn has_pending_network_config(&self) -> bool {
        self.draft_network_profile != self.network_profile
            || self.draft_sequencer_url.trim() != self.sequencer_url
            || self.draft_indexer_url.trim() != self.indexer_url
    }

    fn has_valid_network_config_draft(&self) -> bool {
        has_text(&self.draft_sequencer_url) && has_text(&self.draft_indexer_url)
    }

    fn reset_network_config_draft(&mut self) {
        self.draft_network_profile = self.network_profile.clone();
        self.draft_sequencer_url = self.sequencer_url.clone();
        self.draft_indexer_url = self.indexer_url.clone();
        self.network_config_error = None;
    }

    fn activate_network_config(&mut self) -> bool {
        if !self.has_valid_network_config_draft() {
            self.network_config_error = Some("sequencer and indexer endpoints are required".into());
            return false;
        }

        let sequencer_url = self.draft_sequencer_url.trim().to_owned();
        let indexer_url = self.draft_indexer_url.trim().to_owned();
        let endpoints = match resolve_network_endpoints(
            Some(&self.draft_network_profile),
            Some(&sequencer_url),
            Some(&indexer_url),
        ) {
            Ok(endpoints) => endpoints,
            Err(error) => {
                self.network_config_error = Some(format!("{error:#}"));
                return false;
            }
        };

        let changed = self.network_profile != endpoints.profile
            || self.sequencer_url != endpoints.sequencer_endpoint
            || self.indexer_url != endpoints.indexer_endpoint;
        self.network_profile = endpoints.profile;
        self.sequencer_url = endpoints.sequencer_endpoint;
        self.indexer_url = endpoints.indexer_endpoint;
        self.reset_network_config_draft();
        changed
    }

    fn clear_connection_state(&mut self) {
        self.pending = None;
        self.receiver = None;
        self.overview_receiver = None;
        self.program_ids.clear();
        self.program_ids_error = None;
        self.overview_output = None;
        self.overview_error = None;
        self.output = None;
        self.output_error = None;
        self.result_label = None;
        self.result_scope = None;
        self.last_overview_refresh = None;
        self.last_overview_success = None;
        self.scroll_result_into_view = false;
        self.output_revision = self.output_revision.saturating_add(1);
    }

    fn reconnect_active_node(&mut self, ctx: &egui::Context) {
        self.clear_connection_state();
        self.refresh_overview(ctx);
    }

    fn apply_network_config(&mut self, ctx: &egui::Context) {
        if self.activate_network_config() || !self.has_pending_network_config() {
            self.reconnect_active_node(ctx);
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
            self.refresh_overview(ctx);
        }

        if ctx.egui_wants_keyboard_input() {
            return;
        }

        let selected_view = ctx.input_mut(|input| {
            [
                (egui::Key::Num1, View::Overview),
                (egui::Key::Num2, View::Sequencer),
                (egui::Key::Num3, View::Accounts),
                (egui::Key::Num4, View::Programs),
                (egui::Key::Num5, View::Indexer),
                (egui::Key::Num6, View::Network),
            ]
            .into_iter()
            .find_map(|(key, view)| consume_view_shortcut(input, key).then_some(view))
        });
        if let Some(view) = selected_view {
            self.view = view;
        }
        if ctx.input_mut(|input| consume_command_or_ctrl(input, egui::Key::P)) {
            self.list_program_ids(ctx);
        }
        if ctx.input_mut(|input| consume_view_shortcut(input, egui::Key::ArrowRight)) {
            self.cycle_subtab(true);
        }
        if ctx.input_mut(|input| consume_view_shortcut(input, egui::Key::ArrowLeft)) {
            self.cycle_subtab(false);
        }
        if ctx.input_mut(|input| {
            consume_command_or_ctrl(input, egui::Key::E)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::F12)
        }) {
            self.open_config();
        }
    }

    fn cycle_subtab(&mut self, next: bool) {
        match self.view {
            View::Sequencer => {
                self.sequencer_tab = match (self.sequencer_tab, next) {
                    (SequencerTab::Blocks, true) | (SequencerTab::Transactions, false) => {
                        SequencerTab::Transactions
                    }
                    (SequencerTab::Transactions, true) | (SequencerTab::Blocks, false) => {
                        SequencerTab::Blocks
                    }
                };
            }
            View::Indexer => {
                self.indexer_tab = match (self.indexer_tab, next) {
                    (IndexerTab::Status, true) | (IndexerTab::Rpc, false) => IndexerTab::Rpc,
                    (IndexerTab::Rpc, true) | (IndexerTab::Status, false) => IndexerTab::Status,
                };
            }
            View::Programs => {
                self.programs_tab = match (self.programs_tab, next) {
                    (ProgramsTab::Binaries, true) | (ProgramsTab::Idls, false) => ProgramsTab::Idls,
                    (ProgramsTab::Idls, true) | (ProgramsTab::Events, false) => ProgramsTab::Events,
                    (ProgramsTab::Events, true) | (ProgramsTab::Binaries, false) => {
                        ProgramsTab::Binaries
                    }
                };
            }
            View::Overview | View::Accounts | View::Network => {}
        }
    }

    fn open_config(&mut self) {
        self.view = View::Network;
        self.output_revision = self.output_revision.saturating_add(1);
        self.output = None;
        self.output_error = None;
        self.result_label = None;
        self.result_scope = None;
    }

    fn refresh_overview(&mut self, ctx: &egui::Context) {
        if self.overview_receiver.is_some() {
            return;
        }
        self.last_overview_refresh = Some(Instant::now());
        let sequencer = self.sequencer_url.clone();
        let indexer = self.indexer_url.clone();
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        self.overview_receiver = Some(receiver);
        thread::spawn(move || {
            let result = run_async(async move { dashboard_report(&sequencer, &indexer).await })
                .and_then(|value| serde_json::to_value(value).map_err(Into::into))
                .map_err(|err| format!("{err:#}"));
            if sender.send(result).is_ok() {
                ctx.request_repaint();
            }
        });
    }

    fn list_program_ids(&mut self, ctx: &egui::Context) {
        if self.is_busy() {
            return;
        }
        self.view = View::Programs;
        self.programs_tab = ProgramsTab::Idls;
        let endpoint = self.sequencer_url.clone();
        self.spawn("fetching programs", ctx, move || {
            run_async(async move { sequencer_program_ids(&endpoint).await })
        });
    }

    fn maybe_refresh_overview(&mut self, ctx: &egui::Context) {
        if self.view != View::Overview {
            return;
        }
        ctx.request_repaint_after(OVERVIEW_REFRESH_INTERVAL);
        let should_refresh = self
            .last_overview_refresh
            .is_none_or(|last_refresh| last_refresh.elapsed() >= OVERVIEW_REFRESH_INTERVAL);
        if should_refresh {
            self.refresh_overview(ctx);
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(SIDE_BG)
            .inner_margin(egui::Margin::symmetric(20, 24))
            .show(ui, |ui| {
                ui.set_width((SIDEBAR_WIDTH - 40.0).max(0.0));
                egui::ScrollArea::vertical()
                    .id_salt("sidebar-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if !use_custom_chrome_runtime() {
                            brand(ui, self.pending.as_deref().unwrap_or("ready"));
                            ui.add_space(22.0);
                        }
                        for (view, label) in View::ALL {
                            if nav_item(ui, self.view == view, label).clicked() {
                                self.view = view;
                            }
                            ui.add_space(4.0);
                        }
                        ui.add_space(16.0);
                        divider(ui);
                        ui.add_space(16.0);
                        self.connection_summary(ui);
                    });
            });
    }

    fn compact_sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(SIDE_BG)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if !use_custom_chrome_runtime() {
                    brand(ui, self.pending.as_deref().unwrap_or("ready"));
                    ui.add_space(14.0);
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    for (view, label) in View::ALL {
                        if compact_nav_item(ui, self.view == view, label).clicked() {
                            self.view = view;
                        }
                    }
                });
                ui.add_space(14.0);
                divider(ui);
                ui.add_space(12.0);
                self.connection_summary(ui);
            });
    }

    fn main_content(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(BG)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("main-scroll")
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        centered_content(ui, MAIN_MAX_WIDTH, |ui| {
                            self.topbar(ui);
                            ui.add_space(20.0);
                            self.view_card(ui);
                            ui.add_space(28.0);
                        });
                    });
            });
    }

    fn window_chrome(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(SIDE_BG)
            .inner_margin(egui::Margin::symmetric(14, 7))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
                    compact_brand_mark(ui);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Logos Inspector")
                                .size(14.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(
                            egui::RichText::new(self.view.title())
                                .size(12.0)
                                .color(TEXT_MUTED),
                        );
                    });

                    let reserved = if ui.available_width() > 620.0 {
                        358.0
                    } else {
                        172.0
                    };
                    let drag_width = (ui.available_width() - reserved).max(80.0);
                    let (drag_rect, drag_response) =
                        ui.allocate_exact_size(egui::vec2(drag_width, 34.0), egui::Sense::DRAG);
                    if drag_response.drag_started() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    ui.painter().rect_filled(drag_rect.shrink(1.0), 8, SIDE_BG);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        window_controls(ui);
                        if let Some(pending) = &self.pending {
                            status_spinner(ui, pending);
                        } else {
                            status_pill(ui, "Ready");
                        }
                        if ui.available_width() > 150.0 {
                            let profile_label = network_profile_label(&self.network_profile);
                            tag_pill(ui, profile_label);
                        }
                    });
                });
            });
    }

    fn resize_handles(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let edge = RESIZE_EDGE_THICKNESS;
        let corner = RESIZE_CORNER_SIZE;
        let right = rect.right();
        let bottom = rect.bottom();

        self.resize_handle(
            ui,
            "resize-south",
            egui::Rect::from_min_max(
                egui::pos2(rect.left() + corner, bottom - edge),
                egui::pos2(right - corner, bottom),
            ),
            ResizeAxis::South,
            egui::CursorIcon::ResizeSouth,
        );
        self.resize_handle(
            ui,
            "resize-east",
            egui::Rect::from_min_max(
                egui::pos2(right - edge, rect.top() + corner),
                egui::pos2(right, bottom - corner),
            ),
            ResizeAxis::East,
            egui::CursorIcon::ResizeEast,
        );
        self.resize_handle(
            ui,
            "resize-south-east",
            egui::Rect::from_min_max(
                egui::pos2(right - corner, bottom - corner),
                egui::pos2(right, bottom),
            ),
            ResizeAxis::SouthEast,
            egui::CursorIcon::ResizeSouthEast,
        );
    }

    fn resize_handle(
        &mut self,
        ui: &mut egui::Ui,
        salt: &'static str,
        rect: egui::Rect,
        axis: ResizeAxis,
        cursor: egui::CursorIcon,
    ) {
        let response = ui
            .interact(rect, ui.make_persistent_id(salt), egui::Sense::DRAG)
            .on_hover_and_drag_cursor(cursor);
        if response.drag_started() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::BeginResize(
                    axis.viewport_direction(),
                ));
        }
    }

    fn topbar(&mut self, ui: &mut egui::Ui) {
        if use_custom_chrome_runtime() {
            title_stack(ui, self.view.title());
            return;
        }

        if ui.available_width() < 640.0 {
            ui.vertical(|ui| {
                title_stack(ui, self.view.title());
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    let profile_label = network_profile_label(&self.network_profile).to_owned();
                    tag_pill(ui, &profile_label);
                    if let Some(pending) = &self.pending {
                        status_spinner(ui, pending);
                    } else {
                        status_pill(ui, "Ready");
                    }
                });
            });
            return;
        }

        ui.horizontal(|ui| {
            title_stack(ui, self.view.title());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::BOTTOM), |ui| {
                if let Some(pending) = &self.pending {
                    status_spinner(ui, pending);
                } else {
                    status_pill(ui, "Ready");
                }
                ui.add_space(8.0);
                let profile_label = network_profile_label(&self.network_profile).to_owned();
                tag_pill(ui, &profile_label);
            });
        });
    }

    fn endpoint_controls(&mut self, ui: &mut egui::Ui) {
        let previous_profile = self.draft_network_profile.clone();
        network_profile_field(ui, &mut self.draft_network_profile);
        if self.draft_network_profile != previous_profile
            && self.draft_network_profile != CUSTOM_NETWORK_PROFILE
        {
            self.apply_network_profile_to_draft();
        }
        ui.add_space(12.0);
        let mut endpoints_edited = false;
        endpoints_edited |= endpoint_field(
            ui,
            "sequencer-endpoint",
            "Sequencer",
            &mut self.draft_sequencer_url,
        );
        ui.add_space(12.0);
        endpoints_edited |= endpoint_field(
            ui,
            "indexer-endpoint",
            "Indexer",
            &mut self.draft_indexer_url,
        );
        if endpoints_edited {
            self.draft_network_profile = CUSTOM_NETWORK_PROFILE.to_owned();
            self.network_config_error = None;
        }
        if let Some(error) = &self.network_config_error {
            ui.add_space(12.0);
            error_panel(ui, error);
        }
        ui.add_space(14.0);
        let has_pending_config = self.has_pending_network_config();
        let can_apply = self.has_valid_network_config_draft();
        action_row(ui, |ui| {
            let label = if has_pending_config {
                "Apply"
            } else {
                "Reconnect"
            };
            if primary_button_enabled(ui, label, can_apply).clicked() {
                self.apply_network_config(ui.ctx());
            }
            if secondary_button_enabled(ui, "Reset", has_pending_config).clicked() {
                self.reset_network_config_draft();
            }
        });
    }

    fn connection_summary(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            sidebar_section_label(ui, "Connection");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                status_dot(ui, self.pending.is_none());
            });
        });
        ui.add_space(8.0);
        tag_pill(ui, network_profile_label(&self.network_profile));
        ui.add_space(10.0);
        sidebar_kv(ui, "Sequencer", &compact_endpoint(&self.sequencer_url));
        sidebar_kv(ui, "Indexer", &compact_endpoint(&self.indexer_url));
        ui.add_space(10.0);
        sidebar_kv(ui, "IDL source", &self.active_idl_label());
    }

    fn view_card(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| match self.view {
            View::Overview => self.overview(ui),
            View::Sequencer => self.sequencer(ui),
            View::Accounts => self.account(ui),
            View::Programs => self.programs(ui),
            View::Indexer => self.indexer(ui),
            View::Network => self.config(ui),
        });
    }

    fn overview(&mut self, ui: &mut egui::Ui) {
        self.dashboard_header(ui);
        ui.add_space(12.0);
        let sequencer_head = self.probe_result_text("sequencer", "head");
        let indexer_head = self.probe_result_text("indexer", "head");
        let head_gap = head_gap_text(&sequencer_head, &indexer_head);
        let warnings = self.dashboard_warning_count().to_string();
        self.overview_health_strip(ui, &sequencer_head, &indexer_head, &head_gap, &warnings);
        ui.add_space(12.0);
        self.dashboard(ui);
    }

    fn dashboard_header(&mut self, ui: &mut egui::Ui) {
        let target = dashboard_search_target(&self.dashboard_search);
        let target_label = target
            .as_ref()
            .map(dashboard_search_target_label)
            .unwrap_or("Search");
        let idle = !self.is_busy();
        let mut submit = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("Dashboard")
                    .size(18.0)
                    .strong()
                    .color(TEXT),
            );
            let search_width = (ui.available_width() - 190.0).clamp(260.0, 620.0);
            let response = ui.add_sized(
                [search_width, 38.0],
                egui::TextEdit::singleline(&mut self.dashboard_search)
                    .id_salt("dashboard-search")
                    .hint_text("transaction, account, block"),
            );
            focus_outline(ui, &response);
            submit |=
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            status_pill(ui, target_label);
            submit |= primary_button_enabled(ui, "Search", idle && target.is_some()).clicked();
        });
        if submit && let Some(target) = target {
            self.open_lookup_target(target, ui.ctx());
        }
    }

    fn overview_health_strip(
        &self,
        ui: &mut egui::Ui,
        sequencer_head: &str,
        indexer_head: &str,
        head_gap: &str,
        warnings: &str,
    ) {
        panel(ui).show(ui, |ui| {
            panel_head(ui, "Network health", |ui| {
                status_chip(ui, &self.overview_status_text());
            });
            ui.add_space(10.0);
            let updated = self.last_refresh_text();
            let stats = [
                ("Sequencer head", sequencer_head),
                ("Indexer finalized", indexer_head),
                ("Finality", head_gap),
                ("Warnings", warnings),
                ("Updated", updated.as_str()),
            ];
            compact_stat_grid(ui, &stats);
        });
    }

    fn dashboard(&mut self, ui: &mut egui::Ui) {
        if self.overview_output.is_none()
            && let Some(error) = &self.overview_error
        {
            panel(ui).show(ui, |ui| {
                panel_head(ui, "Dashboard", |_| {});
                ui.add_space(12.0);
                error_panel(ui, error);
            });
            return;
        }

        let Some(output) = self.dashboard_output() else {
            panel(ui).show(ui, |ui| {
                panel_head(ui, "Dashboard", |_| {});
                ui.add_space(12.0);
                dashboard_empty(ui, "Waiting for the first refresh");
            });
            return;
        };
        let output = output.clone();

        dashboard_network_summary(
            ui,
            &output,
            network_profile_label(&self.network_profile),
            &self.sequencer_url,
            &self.indexer_url,
        );
        ui.add_space(16.0);

        let mut selected_block = None;
        let mut selected_transaction = None;
        if ui.available_width() >= DASHBOARD_TWO_COLUMN_MIN_WIDTH {
            ui.columns(2, |columns| {
                if let [blocks, transactions] = columns {
                    selected_block = dashboard_blocks(blocks, &output);
                    selected_transaction = dashboard_transactions(transactions, &output);
                }
            });
        } else {
            selected_block = dashboard_blocks(ui, &output);
            ui.add_space(16.0);
            selected_transaction = dashboard_transactions(ui, &output);
        }

        if let Some(hash) = selected_transaction {
            self.open_dashboard_transaction(hash, ui.ctx());
        } else if let Some(block_id) = selected_block {
            self.open_dashboard_block(block_id, ui.ctx());
        }

        let errors = output
            .get("block_errors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            ui.add_space(16.0);
            panel(ui).show(ui, |ui| {
                panel_head(ui, "Dashboard warnings", |_| {});
                ui.add_space(12.0);
                for error in errors {
                    ui.label(egui::RichText::new(error).color(TEXT_MUTED));
                }
            });
        }
    }

    fn open_dashboard_block(&mut self, block_id: u64, ctx: &egui::Context) {
        if self.is_busy() {
            return;
        }
        self.view = View::Sequencer;
        self.sequencer_tab = SequencerTab::Blocks;
        self.block_id = block_id.to_string();
        let endpoint = self.sequencer_url.clone();
        self.spawn("fetching block", ctx, move || {
            run_async(async move { sequencer_block(&endpoint, block_id).await })
        });
    }

    fn open_dashboard_transaction(&mut self, hash: String, ctx: &egui::Context) {
        if self.is_busy() {
            return;
        }
        self.view = View::Sequencer;
        self.sequencer_tab = SequencerTab::Transactions;
        self.transaction_tab = TransactionTab::Structure;
        self.tx_hash.clone_from(&hash);
        let endpoint = self.sequencer_url.clone();
        self.spawn("inspecting transaction", ctx, move || {
            run_async(async move { sequencer_transaction_inspection(&endpoint, &hash).await })
        });
    }

    fn open_lookup_target(&mut self, target: LookupTarget, ctx: &egui::Context) {
        match target {
            LookupTarget::Account(account_id) => self.open_account_lookup(account_id, ctx),
            LookupTarget::Transaction(hash) => self.open_dashboard_transaction(hash, ctx),
            LookupTarget::Block(block_id) => self.open_dashboard_block(block_id, ctx),
        }
    }

    fn open_account_lookup(&mut self, account_id: String, ctx: &egui::Context) {
        if self.is_busy() {
            return;
        }
        self.view = View::Accounts;
        self.account_tab = AccountTab::Lookup;
        self.account_id.clone_from(&account_id);
        self.run_account_lookup(account_id, ctx);
    }

    fn run_account_lookup(&mut self, account_id: String, ctx: &egui::Context) {
        let sequencer_endpoint = self.sequencer_url.clone();
        let indexer_endpoint = self.indexer_url.clone();
        let idl_json = optional_text(self.account_idl_json.clone());
        let account_type = optional_text(self.account_idl_type.clone());
        if let Some(idl_json) = idl_json {
            self.spawn("fetching account with IDL", ctx, move || {
                run_async(async move {
                    account_lookup_with_idl(
                        &sequencer_endpoint,
                        &indexer_endpoint,
                        &account_id,
                        &idl_json,
                        account_type.as_deref(),
                    )
                    .await
                })
            });
        } else {
            self.spawn("fetching account", ctx, move || {
                run_async(async move {
                    account_lookup(&sequencer_endpoint, &indexer_endpoint, &account_id).await
                })
            });
        }
    }

    fn sequencer(&mut self, ui: &mut egui::Ui) {
        tab_bar(ui, &mut self.sequencer_tab, &SequencerTab::ALL);
        ui.add_space(14.0);
        self.sequencer_controls(ui);
        ui.add_space(18.0);
        self.output_inline(ui, "Sequencer detail");
    }

    fn indexer(&mut self, ui: &mut egui::Ui) {
        tab_bar(ui, &mut self.indexer_tab, &IndexerTab::ALL);
        ui.add_space(14.0);
        if ui.available_width() >= 900.0 {
            screen_split(ui, |ui, controls_width, detail_width| {
                split_panel(ui, controls_width, |ui| self.indexer_controls(ui));
                split_panel(ui, detail_width, |ui| {
                    self.output_card(ui, "Indexer detail")
                });
            });
        } else {
            self.indexer_controls(ui);
            ui.add_space(16.0);
            self.output_card(ui, "Indexer detail");
        }
    }

    fn programs(&mut self, ui: &mut egui::Ui) {
        tab_bar(ui, &mut self.programs_tab, &ProgramsTab::ALL);
        ui.add_space(14.0);
        if ui.available_width() >= 900.0 {
            screen_split(ui, |ui, controls_width, detail_width| {
                split_panel(ui, controls_width, |ui| self.program_controls(ui));
                split_panel(ui, detail_width, |ui| match self.programs_tab {
                    ProgramsTab::Idls => self.idl_detail(ui),
                    ProgramsTab::Binaries | ProgramsTab::Events => {
                        self.output_card(ui, "Program detail");
                    }
                });
            });
        } else {
            self.program_controls(ui);
            ui.add_space(16.0);
            match self.programs_tab {
                ProgramsTab::Idls => self.idl_detail(ui),
                ProgramsTab::Binaries | ProgramsTab::Events => {
                    self.output_card(ui, "Program detail")
                }
            }
        }
    }

    fn sequencer_controls(&mut self, ui: &mut egui::Ui) {
        match self.sequencer_tab {
            SequencerTab::Blocks => self.blocks(ui),
            SequencerTab::Transactions => self.transactions(ui),
        }
    }

    fn indexer_controls(&mut self, ui: &mut egui::Ui) {
        match self.indexer_tab {
            IndexerTab::Status => self.indexer_status(ui),
            IndexerTab::Rpc => self.indexer_rpc(ui),
        }
    }

    fn program_controls(&mut self, ui: &mut egui::Ui) {
        match self.programs_tab {
            ProgramsTab::Binaries => self.program(ui),
            ProgramsTab::Idls => self.idl_registry(ui),
            ProgramsTab::Events => self.event_decode_panel(ui, !self.is_busy()),
        }
    }

    fn config(&mut self, ui: &mut egui::Ui) {
        panel(ui).show(ui, |ui| {
            panel_head(ui, "Network", |_| {});
            ui.add_space(12.0);
            self.endpoint_controls(ui);
        });
    }

    fn blocks(&mut self, ui: &mut egui::Ui) {
        let idle = !self.is_busy();
        panel_head(ui, "Block inspector", |_| {});
        ui.add_space(12.0);
        inline_input_action_row(ui, |ui, input_width, stacked| {
            ui.scope(|ui| {
                ui.set_width(input_width);
                labeled_singleline(ui, "block-id", "Block ID", &mut self.block_id, "7067");
            });
            ui.add_space(if stacked { 10.0 } else { 8.0 });
            if primary_button_enabled(ui, "Inspect", idle && has_text(&self.block_id)).clicked() {
                let endpoint = self.sequencer_url.clone();
                let block_id = self.block_id.clone();
                self.spawn("fetching block", ui.ctx(), move || {
                    let block_id = block_id
                        .parse::<u64>()
                        .with_context(|| format!("invalid block id `{block_id}`"))?;
                    run_async(async move { sequencer_block(&endpoint, block_id).await })
                });
            }
        });
    }

    fn transactions(&mut self, ui: &mut egui::Ui) {
        let idle = !self.is_busy();
        panel_head(ui, "Transaction inspector", |_| {});
        ui.add_space(12.0);
        self.idl_source_selector(ui, "transaction-idl-source");
        ui.add_space(12.0);
        tab_bar(ui, &mut self.transaction_tab, &TransactionTab::ALL);
        ui.add_space(12.0);
        inline_input_action_row(ui, |ui, input_width, stacked| {
            ui.scope(|ui| {
                ui.set_width(input_width);
                labeled_singleline(ui, "tx-hash", "Hash", &mut self.tx_hash, "transaction hash");
            });
            ui.add_space(if stacked { 10.0 } else { 8.0 });
            let can_inspect = idle && has_text(&self.tx_hash);
            if primary_button_enabled(ui, self.transaction_tab.action_label(), can_inspect)
                .clicked()
            {
                self.run_transaction_action(ui.ctx());
            }
        });
        ui.add_space(10.0);
        egui::CollapsingHeader::new("Advanced IDL override")
            .id_salt("transaction-idl")
            .default_open(false)
            .show(ui, |ui| {
                ui.add_space(6.0);
                labeled_multiline(
                    ui,
                    "transaction-idl-json",
                    "IDL JSON",
                    &mut self.transaction_idl_json,
                    6,
                );
            });
    }

    fn run_transaction_action(&mut self, ctx: &egui::Context) {
        let endpoint = self.sequencer_url.clone();
        let hash = self.tx_hash.clone();
        let idl_json = optional_text(self.transaction_idl_json.clone());
        match self.transaction_tab {
            TransactionTab::Summary => {
                self.spawn("fetching transaction", ctx, move || {
                    run_async(async move { sequencer_transaction(&endpoint, &hash).await })
                });
            }
            TransactionTab::Structure => {
                if let Some(idl_json) = idl_json {
                    self.spawn("inspecting transaction with IDL", ctx, move || {
                        run_async(async move {
                            sequencer_transaction_inspection_with_idl(&endpoint, &hash, &idl_json)
                                .await
                        })
                    });
                } else {
                    self.spawn("inspecting transaction", ctx, move || {
                        run_async(async move {
                            sequencer_transaction_inspection(&endpoint, &hash).await
                        })
                    });
                }
            }
            TransactionTab::Trace => {
                if let Some(idl_json) = idl_json {
                    self.spawn("tracing transaction with IDL", ctx, move || {
                        run_async(async move {
                            sequencer_transaction_trace_with_idl(&endpoint, &hash, &idl_json).await
                        })
                    });
                } else {
                    self.spawn("tracing transaction", ctx, move || {
                        run_async(
                            async move { sequencer_transaction_trace(&endpoint, &hash).await },
                        )
                    });
                }
            }
        }
    }

    fn indexer_status(&mut self, ui: &mut egui::Ui) {
        let idle = !self.is_busy();
        panel(ui).show(ui, |ui| {
            panel_head(ui, "Indexer quick calls", |_| {});
            ui.add_space(12.0);
            action_row(ui, |ui| {
                if primary_button_enabled(ui, "Health", idle).clicked() {
                    let endpoint = self.indexer_url.clone();
                    self.spawn("checking indexer", ui.ctx(), move || {
                        run_async(async move {
                            raw_rpc_report(&endpoint, "checkHealth", Value::Array(vec![])).await
                        })
                    });
                }
                if secondary_button_enabled(ui, "Finalized head", idle).clicked() {
                    let endpoint = self.indexer_url.clone();
                    self.spawn("fetching indexer head", ui.ctx(), move || {
                        run_async(async move {
                            raw_rpc_report(
                                &endpoint,
                                "getLastFinalizedBlockId",
                                Value::Array(vec![]),
                            )
                            .await
                        })
                    });
                }
            });
        });
    }

    fn indexer_rpc(&mut self, ui: &mut egui::Ui) {
        let idle = !self.is_busy();
        panel(ui).show(ui, |ui| {
            panel_head(ui, "Custom indexer call", |_| {});
            ui.add_space(12.0);
            labeled_singleline(
                ui,
                "indexer-method",
                "Method",
                &mut self.indexer_method,
                "getLastFinalizedBlockId",
            );
            ui.add_space(10.0);
            labeled_multiline(
                ui,
                "indexer-params",
                "Params JSON",
                &mut self.indexer_params,
                4,
            );
            ui.add_space(12.0);
            let can_call = idle && has_text(&self.indexer_method) && has_text(&self.indexer_params);
            if primary_button_enabled(ui, "Call indexer", can_call).clicked() {
                let endpoint = self.indexer_url.clone();
                let method = self.indexer_method.clone();
                let params = self.indexer_params.clone();
                self.spawn("calling indexer", ui.ctx(), move || {
                    let params: Value = serde_json::from_str(&params)
                        .with_context(|| format!("invalid params JSON `{params}`"))?;
                    run_async(async move { raw_rpc_report(&endpoint, &method, params).await })
                });
            }
        });
    }

    fn account(&mut self, ui: &mut egui::Ui) {
        let idle = !self.is_busy();
        tab_bar(ui, &mut self.account_tab, &AccountTab::ALL);
        ui.add_space(14.0);
        match self.account_tab {
            AccountTab::Lookup => {
                self.account_lookup_header(ui, idle);
                ui.add_space(16.0);
                self.output_inline(ui, "Account detail");
            }
            AccountTab::DecodeData => {
                self.account_decode_controls(ui, idle);
                ui.add_space(16.0);
                self.output_inline(ui, "Account detail");
            }
        }
    }

    fn account_lookup_header(&mut self, ui: &mut egui::Ui, idle: bool) {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("Account lookup")
                    .size(17.0)
                    .strong()
                    .color(TEXT),
            );
            status_pill(ui, &self.account_override_summary());
        });
        ui.add_space(10.0);
        self.account_search_bar(ui, idle);
        ui.add_space(10.0);
        divider(ui);
    }

    fn account_search_bar(&mut self, ui: &mut egui::Ui, idle: bool) {
        let stacked = ui.available_width() < 680.0;
        let mut submit = false;
        let can_lookup = idle && has_text(&self.account_id);
        if stacked {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.account_id)
                    .id_salt("account-id-header")
                    .hint_text("account address")
                    .desired_width(f32::INFINITY),
            );
            focus_outline(ui, &response);
            submit |=
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.add_space(8.0);
            action_row(ui, |ui| {
                submit |= primary_button_enabled(ui, "Lookup account", can_lookup).clicked();
                if secondary_button_enabled(ui, "Override IDL", true).clicked() {
                    self.account_idl_override_open = true;
                }
            });
        } else {
            ui.horizontal(|ui| {
                let action_width = 312.0;
                let input_width = (ui.available_width() - action_width).max(260.0);
                let response = ui.add_sized(
                    [input_width, 38.0],
                    egui::TextEdit::singleline(&mut self.account_id)
                        .id_salt("account-id-header")
                        .hint_text("account address"),
                );
                focus_outline(ui, &response);
                submit |=
                    response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                submit |= primary_button_enabled(ui, "Lookup account", can_lookup).clicked();
                if secondary_button_enabled(ui, "Override IDL", true).clicked() {
                    self.account_idl_override_open = true;
                }
            });
        }
        if submit && can_lookup {
            let account_id = self.account_id.clone();
            self.run_account_lookup(account_id, ui.ctx());
        }
    }

    fn account_decode_controls(&mut self, ui: &mut egui::Ui, idle: bool) {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("Decode account data")
                    .size(17.0)
                    .strong()
                    .color(TEXT),
            );
            status_pill(ui, &self.account_override_summary());
            if secondary_button_enabled(ui, "Override IDL", true).clicked() {
                self.account_idl_override_open = true;
            }
        });
        ui.add_space(12.0);
        labeled_multiline(
            ui,
            "account-data-hex",
            "Data hex",
            &mut self.account_data_hex,
            3,
        );
        ui.add_space(12.0);
        let can_decode =
            idle && has_text(&self.account_idl_json) && has_text(&self.account_data_hex);
        if primary_button_enabled(ui, "Decode data", can_decode).clicked() {
            let idl_json = self.account_idl_json.clone();
            let account_type = optional_text(self.account_idl_type.clone());
            let data_hex = self.account_data_hex.clone();
            self.spawn("decoding account data", ui.ctx(), move || {
                decode_account_data_hex_with_idl(
                    &idl_json,
                    account_type.as_deref(),
                    &data_hex,
                    None,
                )
            });
        }
        ui.add_space(10.0);
        divider(ui);
    }

    fn account_override_summary(&self) -> String {
        if self.account_idl_json.trim().is_empty() {
            return "IDL Auto-detect".to_owned();
        }
        let source = self.active_idl_label();
        let definition_type = account_definition_type_label(&self.account_idl_type);
        format!("IDL {source} / {definition_type}")
    }

    fn account_idl_override_modal(&mut self, ctx: &egui::Context) {
        if !self.account_idl_override_open {
            return;
        }
        let width = (ctx.content_rect().width() - 48.0).clamp(320.0, 620.0);
        let response = egui::Modal::new(egui::Id::new("account-idl-override-modal"))
            .backdrop_color(egui::Color32::from_black_alpha(150))
            .frame(
                egui::Frame::new()
                    .fill(CARD)
                    .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ctx, |ui| {
                let mut close = false;
                ui.set_width(width);
                panel_head(ui, "Override IDL", |ui| {
                    if secondary_button_enabled(ui, "Done", true).clicked() {
                        close = true;
                    }
                });
                ui.add_space(14.0);
                self.idl_source_selector(ui, "account-idl-source-modal");
                ui.add_space(12.0);
                account_definition_type_field(
                    ui,
                    "account-idl-type-modal",
                    &self.account_idl_json,
                    &mut self.account_idl_type,
                );
                ui.add_space(12.0);
                labeled_multiline(
                    ui,
                    "account-idl-json-modal",
                    "IDL JSON",
                    &mut self.account_idl_json,
                    8,
                );
                ui.add_space(14.0);
                action_row(ui, |ui| {
                    if secondary_button_enabled(ui, "Clear override", true).clicked() {
                        self.clear_active_idl();
                    }
                    if primary_button_enabled(ui, "Done", true).clicked() {
                        close = true;
                    }
                });
                close
            });
        if response.inner || response.should_close() {
            self.account_idl_override_open = false;
        }
    }

    fn instruction_decode_panel(&mut self, ui: &mut egui::Ui, idle: bool) {
        panel(ui).show(ui, |ui| {
            panel_head(ui, "IDL inspect", |_| {});
            ui.add_space(12.0);
            self.idl_source_selector(ui, "instruction-idl-source");
            ui.add_space(12.0);
            labeled_singleline(
                ui,
                "instruction-program-id",
                "Program ID",
                &mut self.instruction_program_id,
                "program address",
            );
            ui.add_space(10.0);
            labeled_multiline(
                ui,
                "instruction-words",
                "Instruction words",
                &mut self.instruction_words,
                3,
            );
            ui.add_space(10.0);
            labeled_multiline(
                ui,
                "instruction-accounts",
                "Account IDs",
                &mut self.instruction_accounts,
                3,
            );
            ui.add_space(10.0);
            labeled_multiline(
                ui,
                "instruction-idl-json",
                "IDL JSON",
                &mut self.instruction_idl_json,
                6,
            );
            ui.add_space(12.0);
            let can_decode = idle
                && has_text(&self.instruction_program_id)
                && has_text(&self.instruction_words)
                && has_text(&self.instruction_idl_json);
            if primary_button_enabled(ui, "Decode instruction", can_decode).clicked() {
                let idl_json = self.instruction_idl_json.clone();
                let program_id = self.instruction_program_id.clone();
                let words = self.instruction_words.clone();
                let accounts = self.instruction_accounts.clone();
                self.spawn("decoding instruction", ui.ctx(), move || {
                    let words = parse_words(&words)?;
                    let accounts = parse_accounts(&accounts)?;
                    decode_instruction_words_with_idl(&idl_json, &program_id, &words, &accounts)
                });
            }
        });
    }

    fn idl_registry(&mut self, ui: &mut egui::Ui) {
        let idle = !self.is_busy();
        panel(ui).show(ui, |ui| {
            panel_head(ui, "Program IDL", |_| {});
            ui.add_space(12.0);
            input_action_row(ui, |ui, input_width, stacked| {
                ui.scope(|ui| {
                    ui.set_width(input_width);
                    if input_width >= 560.0 {
                        ui.horizontal(|ui| {
                            let field_width = (input_width - ui.spacing().item_spacing.x) * 0.5;
                            ui.scope(|ui| {
                                ui.set_width(field_width);
                                labeled_singleline(
                                    ui,
                                    "program-idl-program",
                                    "Program ID / label",
                                    &mut self.program_idl_program,
                                    "optional",
                                );
                            });
                            ui.scope(|ui| {
                                ui.set_width(field_width);
                                labeled_singleline(
                                    ui,
                                    "program-idl-name",
                                    "IDL name",
                                    &mut self.program_idl_name,
                                    "auto from JSON",
                                );
                            });
                        });
                    } else {
                        labeled_singleline(
                            ui,
                            "program-idl-program",
                            "Program ID / label",
                            &mut self.program_idl_program,
                            "optional",
                        );
                        ui.add_space(10.0);
                        labeled_singleline(
                            ui,
                            "program-idl-name",
                            "IDL name",
                            &mut self.program_idl_name,
                            "auto from JSON",
                        );
                    }
                });
                ui.add_space(if stacked { 10.0 } else { 8.0 });
                if primary_button_enabled(ui, "Save IDL", idle && has_text(&self.program_idl_json))
                    .clicked()
                {
                    self.register_idl_from_form();
                }
            });
            ui.add_space(10.0);
            labeled_multiline(
                ui,
                "program-idl-json",
                "IDL JSON",
                &mut self.program_idl_json,
                8,
            );
            if let Some(error) = &self.program_idl_error {
                ui.add_space(10.0);
                error_panel(ui, error);
            }
        });

        ui.add_space(16.0);
        panel(ui).show(ui, |ui| {
            panel_head(ui, "Registered IDLs", |_| {});
            ui.add_space(12.0);
            if self.registered_idls.is_empty() {
                ui.label(egui::RichText::new("No IDLs registered").color(TEXT_MUTED));
                return;
            }

            let mut use_index = None;
            let mut remove_index = None;
            for (index, idl) in self.registered_idls.iter().enumerate() {
                idl_registry_row(ui, idl, |ui| {
                    if secondary_button_enabled(ui, "Set active", idle).clicked() {
                        use_index = Some(index);
                    }
                    if secondary_button_enabled(ui, "Remove", idle).clicked() {
                        remove_index = Some(index);
                    }
                });
                ui.add_space(10.0);
            }

            if let Some(index) = use_index
                && let Some(idl) = self.registered_idls.get(index)
            {
                let name = idl.name.clone();
                let json = idl.json.clone();
                self.set_active_idl(name, json);
            }
            if let Some(index) = remove_index
                && let Some(idl) = self.registered_idls.get(index)
            {
                if self
                    .active_idl_name
                    .as_ref()
                    .is_some_and(|name| *name == idl.name)
                {
                    self.active_idl_name = None;
                }
                self.registered_idls.remove(index);
            }
        });

        ui.add_space(16.0);
        panel(ui).show(ui, |ui| {
            panel_head(ui, "Program catalog", |ui| {
                if secondary_button_enabled(ui, "Clear", self.has_program_id_result()).clicked() {
                    self.program_ids.clear();
                    self.program_ids_error = None;
                }
                if secondary_button_enabled(ui, "Copy", self.has_program_id_result()).clicked() {
                    ui.ctx().copy_text(self.copyable_program_ids());
                }
                if primary_button_enabled(ui, "Load", idle).clicked() {
                    self.list_program_ids(ui.ctx());
                }
            });
            ui.add_space(12.0);
            self.program_ids(ui);
        });
    }

    fn idl_detail(&mut self, ui: &mut egui::Ui) {
        let idle = !self.is_busy();
        self.instruction_decode_panel(ui, idle);
        ui.add_space(16.0);
        self.output_card(ui, "IDL inspect result");
    }

    fn has_program_id_result(&self) -> bool {
        !self.program_ids.is_empty() || self.program_ids_error.is_some()
    }

    fn copyable_program_ids(&self) -> String {
        if let Some(error) = &self.program_ids_error {
            return error.clone();
        }
        serde_json::to_string_pretty(&self.program_ids).unwrap_or_default()
    }

    fn register_idl_from_form(&mut self) {
        let json = self.program_idl_json.trim();
        let idl = match serde_json::from_str::<Value>(json) {
            Ok(idl) => idl,
            Err(error) => {
                self.program_idl_error = Some(format!("invalid IDL JSON: {error}"));
                return;
            }
        };
        if !idl.get("instructions").is_some_and(Value::is_array)
            && !idl.get("accounts").is_some_and(Value::is_array)
        {
            self.program_idl_error =
                Some("IDL must contain an instructions or accounts array".to_owned());
            return;
        }

        let name = optional_text(self.program_idl_name.clone())
            .or_else(|| value_str(&idl, "name").map(ToOwned::to_owned))
            .unwrap_or_else(|| format!("IDL {}", self.registered_idls.len() + 1));
        let program_id = optional_text(self.program_idl_program.clone());
        let json = json.to_owned();
        self.registered_idls.push(RegisteredIdl {
            name: name.clone(),
            program_id,
            json: json.clone(),
        });
        self.set_active_idl(name, json);
        self.program_idl_error = None;
    }

    fn set_active_idl(&mut self, name: String, json: String) {
        self.active_idl_name = Some(name);
        self.transaction_idl_json.clone_from(&json);
        self.account_idl_json.clone_from(&json);
        self.instruction_idl_json = json;
        self.program_idl_error = None;
    }

    fn clear_active_idl(&mut self) {
        self.active_idl_name = None;
        self.transaction_idl_json.clear();
        self.account_idl_json.clear();
        self.instruction_idl_json.clear();
        self.account_idl_type.clear();
        self.program_idl_error = None;
    }

    fn idl_source_selector(&mut self, ui: &mut egui::Ui, id_salt: &'static str) {
        let mut selected = self.active_idl_name.clone().unwrap_or_default();
        let current = selected.clone();
        let options = self
            .registered_idls
            .iter()
            .map(|idl| idl.name.clone())
            .collect::<Vec<_>>();

        ui.horizontal_wrapped(|ui| {
            let label_response = ui.label(
                egui::RichText::new("IDL source")
                    .size(13.0)
                    .strong()
                    .color(TEXT_MUTED),
            );
            let response = egui::ComboBox::from_id_salt(id_salt)
                .selected_text(idl_source_label(selected.as_str()))
                .width(ui.available_width().clamp(180.0, 360.0))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, String::new(), "Auto-detect");
                    for name in options {
                        ui.selectable_value(&mut selected, name.clone(), name);
                    }
                })
                .response
                .labelled_by(label_response.id);
            focus_outline(ui, &response);
        });

        if selected == current {
            return;
        }
        if selected.is_empty() {
            self.clear_active_idl();
            return;
        }
        if let Some(idl) = self
            .registered_idls
            .iter()
            .find(|idl| idl.name == selected)
            .cloned()
        {
            self.set_active_idl(idl.name, idl.json);
        }
    }

    fn program_ids(&mut self, ui: &mut egui::Ui) {
        if self.pending.as_deref() == Some("fetching programs") {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 120.0),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(egui::RichText::new("Loading program IDs").color(TEXT_MUTED));
                    });
                },
            );
            return;
        }
        if self.program_ids.is_empty() && self.program_ids_error.is_none() {
            dashboard_empty(ui, "Load program IDs from the sequencer");
            return;
        }

        if let Some(error) = &self.program_ids_error {
            error_panel(ui, error);
            return;
        }

        let mut lookup = None;
        for program in &self.program_ids {
            program_id_row(ui, program, &mut lookup);
            ui.add_space(10.0);
        }
        if let Some(target) = lookup {
            self.open_lookup_target(target, ui.ctx());
        }
    }

    fn event_decode_panel(&mut self, ui: &mut egui::Ui, idle: bool) {
        panel(ui).show(ui, |ui| {
            panel_head(ui, "Event decode", |_| {});
            ui.add_space(12.0);
            self.idl_source_selector(ui, "event-idl-source");
            ui.add_space(12.0);
            labeled_singleline(
                ui,
                "event-name",
                "Event",
                &mut self.event_name,
                "optional event name",
            );
            ui.add_space(10.0);
            labeled_multiline(
                ui,
                "event-data-hex",
                "Event data hex",
                &mut self.event_data_hex,
                3,
            );
            ui.add_space(12.0);
            let can_decode_event =
                idle && has_text(&self.instruction_idl_json) && has_text(&self.event_data_hex);
            if primary_button_enabled(ui, "Decode event", can_decode_event).clicked() {
                let idl_json = self.instruction_idl_json.clone();
                let event_name = optional_text(self.event_name.clone());
                let data_hex = self.event_data_hex.clone();
                self.spawn("decoding event", ui.ctx(), move || {
                    decode_event_data_hex_with_idl(&idl_json, event_name.as_deref(), &data_hex)
                });
            }
        });
    }

    fn program(&mut self, ui: &mut egui::Ui) {
        let idle = !self.is_busy();
        panel(ui).show(ui, |ui| {
            panel_head(ui, "Program file", |_| {});
            ui.add_space(12.0);
            input_action_row(ui, |ui, input_width, stacked| {
                ui.scope(|ui| {
                    ui.set_width(input_width);
                    labeled_singleline(
                        ui,
                        "program-path",
                        "Path",
                        &mut self.program_path,
                        "program.bin",
                    );
                });
                ui.add_space(if stacked { 10.0 } else { 8.0 });
                if primary_button_enabled(ui, "Inspect", idle && has_text(&self.program_path))
                    .clicked()
                {
                    let path = self.program_path.clone();
                    self.spawn("inspecting program", ui.ctx(), move || {
                        program_file_info(path)
                    });
                }
            });
        });
    }

    fn current_result_scope(&self) -> ResultScope {
        match self.view {
            View::Overview => ResultScope::Overview,
            View::Sequencer => ResultScope::Sequencer(self.sequencer_tab),
            View::Accounts => ResultScope::Accounts(self.account_tab),
            View::Programs => ResultScope::Programs(self.programs_tab),
            View::Indexer => ResultScope::Indexer(self.indexer_tab),
            View::Network => ResultScope::Network,
        }
    }

    fn empty_detail_text(&self) -> &'static str {
        match self.current_result_scope() {
            ResultScope::Sequencer(SequencerTab::Blocks) => "Enter a block ID",
            ResultScope::Sequencer(SequencerTab::Transactions) => "Paste a transaction hash",
            ResultScope::Accounts(AccountTab::Lookup) => "Enter an account ID",
            ResultScope::Accounts(AccountTab::DecodeData) => "Paste account data and an IDL",
            ResultScope::Programs(ProgramsTab::Binaries) => "Choose a program binary",
            ResultScope::Programs(ProgramsTab::Idls) => "Decode instruction words with an IDL",
            ResultScope::Programs(ProgramsTab::Events) => "Paste event data and an IDL",
            ResultScope::Indexer(IndexerTab::Status) => "Run an indexer health check",
            ResultScope::Indexer(IndexerTab::Rpc) => "Prepare an indexer RPC call",
            ResultScope::Overview | ResultScope::Network => "Select an explorer item",
        }
    }

    fn output_card(&mut self, ui: &mut egui::Ui, title: &str) {
        self.output_section(ui, title, true);
    }

    fn output_inline(&mut self, ui: &mut egui::Ui, title: &str) {
        self.output_section(ui, title, false);
    }

    fn output_section(&mut self, ui: &mut egui::Ui, title: &str, boxed: bool) {
        let should_scroll_to_result =
            self.scroll_result_into_view && self.result_scope == Some(self.current_result_scope());
        let empty_text = self.empty_detail_text();
        if boxed {
            panel(ui).show(ui, |ui| {
                self.output_section_contents(ui, title, empty_text, should_scroll_to_result, true);
            });
        } else {
            self.output_section_contents(ui, title, empty_text, should_scroll_to_result, false);
        }
        if should_scroll_to_result {
            ui.scroll_to_cursor(Some(egui::Align::Center));
            self.scroll_result_into_view = false;
        }
    }

    fn output_section_contents(
        &mut self,
        ui: &mut egui::Ui,
        title: &str,
        empty_text: &str,
        should_scroll_to_result: bool,
        boxed_result: bool,
    ) {
        let current = self.result_scope == Some(self.current_result_scope());
        let has_result = current && (self.output.is_some() || self.output_error.is_some());
        let current_pending = current && self.pending.is_some();
        if ui.available_width() < 340.0 {
            ui.label(egui::RichText::new(title).size(16.0).strong().color(TEXT));
            ui.add_space(8.0);
            action_row(ui, |ui| {
                if secondary_button_enabled(ui, "Copy", has_result).clicked() {
                    ui.ctx().copy_text(self.copyable_result());
                }
                if secondary_button_enabled(ui, "Clear", has_result).clicked() {
                    self.output = None;
                    self.output_error = None;
                    self.result_label = None;
                    self.result_scope = None;
                }
            });
        } else {
            panel_head(ui, title, |ui| {
                if secondary_button_enabled(ui, "Clear", has_result).clicked() {
                    self.output = None;
                    self.output_error = None;
                    self.result_label = None;
                    self.result_scope = None;
                }
                if secondary_button_enabled(ui, "Copy data", has_result).clicked() {
                    ui.ctx().copy_text(self.copyable_result());
                }
            });
        }
        if current && let Some(label) = self.result_label.as_ref() {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(label)
                    .size(13.0)
                    .strong()
                    .color(TEXT_MUTED),
            );
        }
        ui.add_space(10.0);
        let result_scroll_id = format!(
            "result-scroll:{}:{}:{}",
            std::process::id(),
            self.output_revision,
            self.result_label.as_deref().unwrap_or("result")
        );
        let result_area = if boxed_result {
            egui::Frame::new()
                .fill(PANEL)
                .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    self.output_result_contents(
                        ui,
                        current,
                        current_pending,
                        empty_text,
                        &result_scroll_id,
                        true,
                    );
                })
        } else {
            ui.vertical(|ui| {
                self.output_result_contents(
                    ui,
                    current,
                    current_pending,
                    empty_text,
                    &result_scroll_id,
                    false,
                );
            })
        };
        let result_response = ui.interact(
            result_area.response.rect,
            ui.make_persistent_id((
                "result-status",
                format!("{:?}", self.current_result_scope()),
                self.output_revision,
            )),
            egui::Sense::focusable_noninteractive(),
        );
        result_response.widget_info(|| {
            let label = if current && self.output_error.is_some() {
                format!("{title}: error")
            } else if current && self.output.is_some() {
                format!("{title}: result loaded")
            } else {
                format!("{title}: {empty_text}")
            };
            egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label)
        });
        focus_outline(ui, &result_response);
        if should_scroll_to_result {
            result_response.request_focus();
        }
    }

    fn output_result_contents(
        &mut self,
        ui: &mut egui::Ui,
        current: bool,
        current_pending: bool,
        empty_text: &str,
        result_scroll_id: &str,
        nested_scroll: bool,
    ) {
        ui.set_min_width(ui.available_width());
        if current_pending && self.output.is_none() && self.output_error.is_none() {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 160.0),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("Loading result")
                                .size(14.0)
                                .color(TEXT_MUTED),
                        );
                    });
                },
            );
        } else if !current || (self.output.is_none() && self.output_error.is_none()) {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 160.0),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    ui.label(egui::RichText::new(empty_text).size(14.0).color(TEXT_MUTED));
                },
            );
        } else if let Some(error) = &self.output_error {
            if nested_scroll && result_error_needs_scroll(error) {
                result_scroll_area(ui, result_scroll_id, |ui| error_panel(ui, error));
            } else {
                error_panel(ui, error);
            }
        } else if let Some(output) = &self.output {
            let account_output_tab = &mut self.account_output_tab;
            let lookup = if nested_scroll && result_value_needs_scroll(output) {
                result_scroll_area(ui, result_scroll_id, |ui| {
                    render_detail_value(ui, output, account_output_tab)
                })
                .inner
            } else {
                render_detail_value(ui, output, account_output_tab)
            };
            if let Some(target) = lookup {
                self.open_lookup_target(target, ui.ctx());
            }
        } else {
            ui.label(egui::RichText::new("No result").color(TEXT_MUTED));
        }
    }

    fn copyable_result(&self) -> String {
        if let Some(error) = &self.output_error {
            return error.clone();
        }
        self.output
            .as_ref()
            .and_then(|value| serde_json::to_string_pretty(value).ok())
            .unwrap_or_default()
    }

    fn dashboard_output(&self) -> Option<&Value> {
        self.overview_output.as_ref()
    }

    fn active_idl_label(&self) -> String {
        self.active_idl_name
            .as_deref()
            .map_or_else(|| idl_source_label("").to_owned(), |name| name.to_owned())
    }

    fn last_refresh_text(&self) -> String {
        self.last_overview_success
            .map(|refresh| format!("{} ago", duration_label(refresh.elapsed())))
            .unwrap_or_else(|| "Not loaded".to_owned())
    }

    fn dashboard_warning_count(&self) -> usize {
        self.dashboard_output()
            .and_then(|output| output.get("block_errors"))
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0)
    }

    fn probe_result_text(&self, service: &str, field: &str) -> String {
        self.overview_output
            .as_ref()
            .map(overview_payload)
            .and_then(|output| output.get(service))
            .and_then(|service| service.get(field))
            .map(probe_text)
            .unwrap_or_else(|| "-".to_owned())
    }

    fn overview_status_text(&self) -> String {
        if self.pending.is_some() {
            return "Working".to_owned();
        }
        if self.overview_output.is_none() && self.overview_receiver.is_some() {
            return "Updating".to_owned();
        }
        if self.overview_output.is_some() && self.overview_error.is_some() {
            return "Stale".to_owned();
        }
        if self.overview_error.is_some() {
            return "Error".to_owned();
        }
        let Some(output) = &self.overview_output else {
            return "Ready".to_owned();
        };
        let overview = overview_payload(output);
        let sequencer_ok = probe_ok(overview, "sequencer", "health");
        let indexer_ok = probe_ok(overview, "indexer", "health");
        match (sequencer_ok, indexer_ok) {
            (Some(true), Some(true)) => "Healthy".to_owned(),
            (Some(false), _) | (_, Some(false)) => "Error".to_owned(),
            _ => "Ready".to_owned(),
        }
    }

    #[cfg(test)]
    fn has_visible_result(&self) -> bool {
        self.view != View::Overview
            && self.view != View::Network
            && self.result_scope == Some(self.current_result_scope())
            && (self.pending.is_some() || self.output.is_some() || self.output_error.is_some())
    }

    fn spawn<T, F>(&mut self, label: &'static str, ctx: &egui::Context, task: F)
    where
        T: serde::Serialize + Send + 'static,
        F: FnOnce() -> Result<T> + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        self.pending = Some(label.to_owned());
        self.result_label = Some(format_label(label));
        self.result_scope = Some(self.current_result_scope());
        if self.current_result_scope() == ResultScope::Overview {
            self.overview_error = None;
        } else {
            self.output = None;
            self.output_error = None;
        }
        self.output_revision = self.output_revision.saturating_add(1);
        self.scroll_result_into_view = false;
        self.receiver = Some(receiver);
        thread::spawn(move || {
            let result = task()
                .and_then(|value| serde_json::to_value(value).map_err(Into::into))
                .map_err(|err| format!("{err:#}"));
            if sender.send(result).is_ok() {
                ctx.request_repaint();
            }
        });
    }

    fn receive_task(&mut self) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(output)) => {
                let label = self
                    .pending
                    .as_deref()
                    .map(|pending| completion_label(pending, &output));
                if self.pending.as_deref() == Some("loading overview") {
                    self.overview_output = Some(output);
                    self.overview_error = None;
                    self.last_overview_success = Some(Instant::now());
                    self.result_label = label;
                    self.pending = None;
                    self.receiver = None;
                    self.scroll_result_into_view = false;
                    return;
                }
                if self.pending.as_deref() == Some("fetching programs") {
                    self.program_ids = output.as_array().cloned().unwrap_or_default();
                    self.program_ids_error = None;
                    self.output = None;
                    self.output_error = None;
                    self.result_label = None;
                    self.result_scope = None;
                    self.pending = None;
                    self.receiver = None;
                    return;
                }
                if let Some(account_output_tab) = default_account_output_tab(&output) {
                    self.account_output_tab = account_output_tab;
                }
                self.output = Some(output);
                self.output_error = None;
                self.result_label = label;
                self.pending = None;
                self.receiver = None;
                self.scroll_result_into_view = true;
            }
            Ok(Err(error)) => {
                if self.pending.as_deref() == Some("loading overview") {
                    self.overview_error = Some(error);
                    self.result_label = self
                        .pending
                        .as_deref()
                        .map(|pending| format!("{} failed", format_label(pending)));
                    self.pending = None;
                    self.receiver = None;
                    self.scroll_result_into_view = false;
                    return;
                }
                if self.pending.as_deref() == Some("fetching programs") {
                    self.program_ids.clear();
                    self.program_ids_error = Some(error);
                    self.output = None;
                    self.output_error = None;
                    self.result_label = None;
                    self.result_scope = None;
                    self.pending = None;
                    self.receiver = None;
                    return;
                }
                let label = self
                    .pending
                    .as_deref()
                    .map(|pending| format!("{} failed", format_label(pending)));
                self.output = None;
                self.output_error = Some(error);
                self.result_label = label;
                self.pending = None;
                self.receiver = None;
                self.scroll_result_into_view = true;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.output = None;
                self.output_error = Some("background task disconnected".to_owned());
                self.pending = None;
                self.receiver = None;
            }
        }
    }

    fn receive_overview_task(&mut self) {
        let Some(receiver) = &self.overview_receiver else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(output)) => {
                self.overview_output = Some(output);
                self.overview_error = None;
                self.last_overview_success = Some(Instant::now());
                self.overview_receiver = None;
            }
            Ok(Err(error)) => {
                self.overview_error = Some(error);
                self.overview_receiver = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.overview_error = Some("overview refresh disconnected".to_owned());
                self.overview_receiver = None;
            }
        }
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = BG;
    visuals.window_fill = CARD;
    visuals.extreme_bg_color = INPUT;
    visuals.faint_bg_color = PANEL;
    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.bg_fill = PANEL;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_MUTED);
    visuals.widgets.hovered.bg_fill = PANEL_HOVER;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.widgets.active.bg_fill = ACCENT_DARK;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    visuals.selection.bg_fill = ACCENT_DARK;
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, visuals);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(14.0, 8.0);
        style.spacing.interact_size.y = 42.0;
    });
}

fn centered_content<R>(
    ui: &mut egui::Ui,
    max_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let width = ui.available_width().min(max_width);
    let left = ((ui.available_width() - width) * 0.5).max(0.0);
    ui.horizontal(|ui| {
        ui.add_space(left);
        ui.vertical(|ui| {
            ui.set_width(width);
            add_contents(ui)
        })
        .inner
    })
}

fn screen_split(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui, f32, f32)) {
    let width = ui.available_width();
    let controls_width = if width >= 1160.0 { 340.0 } else { 320.0 };
    let detail_width = (width - controls_width - 16.0).max(360.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(16.0, 0.0);
        add_contents(ui, controls_width, detail_width);
    });
}

fn split_panel(ui: &mut egui::Ui, width: f32, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.set_min_width(width);
        add_contents(ui);
    });
}

fn vertical_rule(ui: &mut egui::Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, height), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0, BORDER);
}

fn brand(ui: &mut egui::Ui, status: &str) {
    ui.horizontal(|ui| {
        brand_mark(ui);
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Logos Inspector")
                    .size(18.0)
                    .strong()
                    .color(TEXT),
            );
            ui.label(egui::RichText::new(status).size(12.0).color(TEXT_MUTED));
        });
    });
}

fn brand_mark(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 8, CARD);
    ui.painter().rect_stroke(
        rect,
        8,
        egui::Stroke::new(1.0, BORDER_STRONG),
        egui::StrokeKind::Inside,
    );
    let gap = 4.0;
    let inner = rect.shrink(8.0);
    let col_w = (inner.width() - gap * 2.0) / 3.0;
    for (index, color) in [ACCENT, BORDER_STRONG, GREEN].into_iter().enumerate() {
        let x = inner.left() + (col_w + gap) * index as f32;
        let bar = egui::Rect::from_min_size(
            egui::pos2(x, inner.top()),
            egui::vec2(col_w, inner.height()),
        );
        ui.painter().rect_filled(bar, 2, color);
    }
}

fn title_stack(ui: &mut egui::Ui, title: &str) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new("Blockchain explorer")
                .size(11.0)
                .strong()
                .color(ACCENT),
        );
        ui.add_space(2.0);
        ui.label(egui::RichText::new(title).size(24.0).strong().color(TEXT));
    });
}

fn window_controls(ui: &mut egui::Ui) {
    if !use_custom_chrome_runtime() {
        return;
    }

    if frame_button(ui, "x", "Close", true).clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
    if frame_button(ui, "-", "Minimize", false).clicked() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
}

fn result_scroll_area<R>(
    ui: &mut egui::Ui,
    id_salt: impl egui::AsIdSalt,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::scroll_area::ScrollAreaOutput<R> {
    let max_height = ui.available_height().clamp(320.0, 640.0);
    egui::ScrollArea::both()
        .id_salt(id_salt)
        .max_height(max_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add_contents(ui)
        })
}

fn result_value_needs_scroll(value: &Value) -> bool {
    estimated_result_rows(value) > 10 || estimated_result_text_len(value) > 1_800
}

fn result_error_needs_scroll(error: &str) -> bool {
    error.lines().count() > 8 || error.len() > 1_000
}

fn estimated_result_rows(value: &Value) -> usize {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 1,
        Value::Array(items) => {
            if items.is_empty() {
                1
            } else if items.iter().all(is_scalar) {
                items.len()
            } else if items.iter().all(is_scalar_object) {
                items
                    .iter()
                    .filter_map(Value::as_object)
                    .map(|object| object.len().max(1) + 1)
                    .sum()
            } else {
                items.iter().map(estimated_result_rows).sum::<usize>() + items.len()
            }
        }
        Value::Object(object) => {
            if object.is_empty() {
                1
            } else if object.values().all(is_scalar) {
                object.len()
            } else {
                object.values().map(estimated_result_rows).sum::<usize>() + object.len()
            }
        }
    }
}

fn estimated_result_text_len(value: &Value) -> usize {
    match value {
        Value::Null => 1,
        Value::Bool(value) => {
            if *value {
                3
            } else {
                2
            }
        }
        Value::Number(value) => value.to_string().len(),
        Value::String(value) => value.len(),
        Value::Array(items) => items.iter().map(estimated_result_text_len).sum(),
        Value::Object(object) => object
            .iter()
            .map(|(key, value)| key.len() + estimated_result_text_len(value))
            .sum(),
    }
}

fn paint_resize_grip(ui: &egui::Ui, rect: egui::Rect) {
    let right = rect.right() - 7.0;
    let bottom = rect.bottom() - 7.0;
    for offset in [0.0, 5.0, 10.0] {
        ui.painter().line_segment(
            [
                egui::pos2(right - 18.0 + offset, bottom),
                egui::pos2(right, bottom - 18.0 + offset),
            ],
            egui::Stroke::new(1.0, BORDER_STRONG),
        );
    }
}

fn frame_button(ui: &mut egui::Ui, label: &str, tooltip: &str, close: bool) -> egui::Response {
    let fill = if close { CLOSE_IDLE } else { PANEL };
    let stroke = if close {
        egui::Stroke::new(1.0, BORDER_STRONG)
    } else {
        egui::Stroke::new(1.0, BORDER)
    };
    let response = ui.add(
        egui::Button::new(egui::RichText::new(label).size(13.0).color(TEXT_MUTED))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(8))
            .min_size(egui::vec2(44.0, 44.0)),
    );
    focus_outline(ui, &response);
    response.on_hover_text(tooltip)
}

fn compact_brand_mark(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 7, CARD);
    ui.painter().rect_stroke(
        rect,
        7,
        egui::Stroke::new(1.0, BORDER_STRONG),
        egui::StrokeKind::Inside,
    );
    let gap = 3.0;
    let inner = rect.shrink(7.0);
    let col_w = (inner.width() - gap * 2.0) / 3.0;
    for (index, color) in [ACCENT, BORDER_STRONG, GREEN].into_iter().enumerate() {
        let x = inner.left() + (col_w + gap) * index as f32;
        let bar = egui::Rect::from_min_size(
            egui::pos2(x, inner.top()),
            egui::vec2(col_w, inner.height()),
        );
        ui.painter().rect_filled(bar, 2, color);
    }
}

fn panel(_ui: &egui::Ui) -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(16))
}

fn panel_head(ui: &mut egui::Ui, title: &str, add_actions: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).size(16.0).strong().color(TEXT));
        ui.with_layout(
            egui::Layout::right_to_left(egui::Align::Center),
            add_actions,
        );
    });
}

fn compact_stat(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(11.0)
                .strong()
                .color(TEXT_MUTED),
        );
        ui.add_space(3.0);
        ui.label(egui::RichText::new(value).size(13.0).color(TEXT));
    });
}

fn compact_stat_grid(ui: &mut egui::Ui, stats: &[(&str, &str)]) {
    let columns = if ui.available_width() >= 920.0 {
        stats.len()
    } else if ui.available_width() >= 520.0 {
        2
    } else {
        1
    };

    for row in stats.chunks(columns) {
        ui.columns(columns, |columns| {
            for (column, (label, value)) in columns.iter_mut().zip(row.iter()) {
                compact_stat(column, label, value);
            }
        });
        ui.add_space(10.0);
    }
}

fn idl_source_label(value: &str) -> &str {
    if value.is_empty() {
        "Auto-detect"
    } else {
        value
    }
}

fn account_definition_type_label(value: &str) -> &str {
    if value.trim().is_empty() {
        "Auto-detect"
    } else {
        value
    }
}

fn tab_bar<T>(ui: &mut egui::Ui, selected: &mut T, tabs: &[(T, &'static str)])
where
    T: Copy + PartialEq,
{
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        for (value, label) in tabs.iter().copied() {
            let active = *selected == value;
            let fill = if active { ACCENT_DARK } else { PANEL };
            let stroke = if active {
                egui::Stroke::new(1.0, ACCENT)
            } else {
                egui::Stroke::new(1.0, BORDER)
            };
            let text = if active { TEXT } else { TEXT_MUTED };
            let response = ui.add(
                egui::Button::new(egui::RichText::new(label).size(14.0).strong().color(text))
                    .fill(fill)
                    .stroke(stroke)
                    .corner_radius(egui::CornerRadius::same(8))
                    .min_size(egui::vec2(118.0, 38.0)),
            );
            focus_outline(ui, &response);
            if response.clicked() {
                *selected = value;
            }
        }
    });
}

fn dashboard_network_summary(
    ui: &mut egui::Ui,
    output: &Value,
    network_profile: &str,
    sequencer_endpoint: &str,
    indexer_endpoint: &str,
) {
    if ui.available_width() >= DASHBOARD_TWO_COLUMN_MIN_WIDTH {
        ui.columns(2, |columns| {
            if let [summary, signals] = columns {
                dashboard_topology(
                    summary,
                    output,
                    network_profile,
                    sequencer_endpoint,
                    indexer_endpoint,
                );
                dashboard_operational_signals(signals, output);
            }
        });
    } else {
        dashboard_topology(
            ui,
            output,
            network_profile,
            sequencer_endpoint,
            indexer_endpoint,
        );
        ui.add_space(16.0);
        dashboard_operational_signals(ui, output);
    }
}

fn dashboard_topology(
    ui: &mut egui::Ui,
    output: &Value,
    network_profile: &str,
    sequencer_endpoint: &str,
    indexer_endpoint: &str,
) {
    panel(ui).show(ui, |ui| {
        panel_head(ui, "Network topology", |_| {});
        ui.add_space(12.0);
        let overview = overview_payload(output);
        let sequencer_health = probe_text_field(overview, "sequencer", "health");
        let indexer_health = probe_text_field(overview, "indexer", "health");
        let sequencer_head = probe_text_field(overview, "sequencer", "head");
        let indexer_head = probe_text_field(overview, "indexer", "head");
        let finality = head_gap_text(&sequencer_head, &indexer_head);
        let sequencer = compact_endpoint(sequencer_endpoint);
        let indexer = compact_endpoint(indexer_endpoint);
        let topology = "Inspector -> Sequencer RPC; Inspector -> Indexer RPC";
        let stats = [
            ("Network", network_profile),
            ("Topology", topology),
            ("Sequencer", sequencer.as_str()),
            ("Indexer", indexer.as_str()),
            ("Sequencer health", sequencer_health.as_str()),
            ("Indexer health", indexer_health.as_str()),
            ("Current block", sequencer_head.as_str()),
            ("Latest finalized", indexer_head.as_str()),
            ("Finality lag", finality.as_str()),
        ];
        compact_stat_grid(ui, &stats);
    });
}

fn dashboard_operational_signals(ui: &mut egui::Ui, output: &Value) {
    panel(ui).show(ui, |ui| {
        panel_head(ui, "Network signals", |_| {});
        ui.add_space(12.0);
        let recent_txs = value_usize(output, "recent_transaction_count")
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_owned());
        let tps = output
            .get("recent_tps")
            .and_then(Value::as_f64)
            .map(format_tps)
            .unwrap_or_else(|| "-".to_owned());
        let window = value_u64(output, "recent_window_seconds")
            .map(|value| format!("{value}s"))
            .unwrap_or_else(|| "-".to_owned());
        dashboard_signal_row(
            ui,
            "Recent transactions",
            &recent_txs,
            "recent finalized window",
        );
        dashboard_signal_row(ui, "Transactions per second", &tps, &window);
        dashboard_signal_row(
            ui,
            "Pending transactions",
            "Unavailable",
            "not exposed by RPC",
        );
        dashboard_signal_row(ui, "Mempool", "Unavailable", "not exposed by RPC");
        dashboard_signal_row(
            ui,
            "Active block producers",
            "Unavailable",
            "not exposed by RPC",
        );
        dashboard_signal_row(ui, "Connected nodes", "Unavailable", "not exposed by RPC");
    });
}

fn dashboard_signal_row(ui: &mut egui::Ui, label: &str, value: &str, source: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.add_sized(
            [158.0, 24.0],
            egui::Label::new(
                egui::RichText::new(label)
                    .size(12.0)
                    .strong()
                    .color(TEXT_MUTED),
            ),
        );
        ui.add_sized(
            [128.0, 24.0],
            egui::Label::new(egui::RichText::new(value).size(13.0).strong().color(TEXT)).truncate(),
        )
        .on_hover_text(value);
        ui.label(egui::RichText::new(source).size(12.0).color(TEXT_MUTED));
    });
    ui.add_space(8.0);
}

fn dashboard_blocks(ui: &mut egui::Ui, output: &Value) -> Option<u64> {
    let mut selected = None;
    panel(ui).show(ui, |ui| {
        panel_head(ui, "Latest blocks", |_| {});
        ui.add_space(12.0);
        let Some(blocks) = output.get("latest_blocks").and_then(Value::as_array) else {
            dashboard_empty(ui, "No block data yet");
            return;
        };
        if blocks.is_empty() {
            dashboard_empty(ui, "No block data yet");
            return;
        }

        for block in blocks.iter().take(DASHBOARD_BLOCK_LIMIT) {
            if dashboard_block_row(ui, block) {
                selected = value_u64(block, "block_id");
            }
            ui.add_space(10.0);
        }
    });
    selected
}

fn dashboard_block_row(ui: &mut egui::Ui, block: &Value) -> bool {
    let block_id = value_u64(block, "block_id")
        .map(|value| format!("#{value}"))
        .unwrap_or_else(|| "-".to_owned());
    let status = value_str(block, "bedrock_status").unwrap_or("unknown");
    let timestamp = value_u64(block, "timestamp")
        .map(timestamp_label)
        .unwrap_or_else(|| "-".to_owned());
    let tx_count = value_usize(block, "tx_count")
        .map(|value| format!("{value} tx"))
        .unwrap_or_else(|| "-".to_owned());

    let id = ui.make_persistent_id(("dashboard-block-row", block_id.as_str()));
    let inner = egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(58.0);
            if ui.available_width() < DASHBOARD_COMPACT_ROW_WIDTH {
                ui.horizontal_wrapped(|ui| {
                    row_cell(ui, &block_id, 76.0, true);
                    row_cell(ui, &timestamp, 132.0, false);
                    status_chip(ui, status);
                    dashboard_row_action(ui);
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    row_cell(ui, &tx_count, 72.0, false);
                });
            } else {
                ui.horizontal(|ui| {
                    row_cell(ui, &block_id, 76.0, true);
                    row_cell(ui, &timestamp, 132.0, false);
                    row_cell(ui, &tx_count, 72.0, false);
                    status_chip(ui, status);
                    let spacer = (ui.available_width() - 44.0).max(0.0);
                    ui.add_space(spacer);
                    dashboard_row_action(ui);
                });
            }
            if let Some(warning) = value_str(block, "decode_warning") {
                ui.add_space(8.0);
                dashboard_warning_row(ui, "Warning", warning);
            }
        });
    let response = ui
        .interact(inner.response.rect, id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            true,
            format!("Open block {block_id}"),
        )
    });
    focus_outline(ui, &response);
    response.clicked()
}

fn dashboard_transactions(ui: &mut egui::Ui, output: &Value) -> Option<String> {
    let mut selected = None;
    panel(ui).show(ui, |ui| {
        panel_head(ui, "Latest transactions", |_| {});
        ui.add_space(12.0);
        let Some(transactions) = output.get("latest_transactions").and_then(Value::as_array) else {
            dashboard_empty(ui, "No transaction data yet");
            return;
        };
        if transactions.is_empty() {
            dashboard_empty(ui, "No transaction data yet");
            return;
        }

        for transaction in transactions.iter().take(DASHBOARD_TRANSACTION_LIMIT) {
            if dashboard_transaction_row(ui, transaction)
                && let Some(hash) = value_str(transaction, "hash")
            {
                selected = Some(hash.to_owned());
            }
            ui.add_space(10.0);
        }
    });
    selected
}

fn dashboard_transaction_row(ui: &mut egui::Ui, transaction: &Value) -> bool {
    let hash = value_str(transaction, "hash").unwrap_or("-");
    let kind = value_str(transaction, "kind").unwrap_or("unknown");
    let block_id = value_u64(transaction, "block_id")
        .map(|value| format!("#{value}"))
        .unwrap_or_else(|| "-".to_owned());
    let account_count = value_usize(transaction, "account_count")
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_owned());
    let instruction_words = value_usize(transaction, "instruction_words")
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_owned());
    let program = value_str(transaction, "program_id_hex")
        .map(short_token)
        .unwrap_or_else(|| "-".to_owned());
    let account_label = format!("{account_count} accts");
    let words_label = format!("{instruction_words} words");

    let id = ui.make_persistent_id(("dashboard-transaction-row", hash));
    let inner = egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(66.0);
            let hash_label = short_token(hash);
            if ui.available_width() < DASHBOARD_COMPACT_ROW_WIDTH {
                ui.horizontal_wrapped(|ui| {
                    let hash_width = (ui.available_width() - 128.0).clamp(150.0, 280.0);
                    row_cell(ui, &hash_label, hash_width, true);
                    status_chip(ui, kind);
                    dashboard_row_action(ui);
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    row_cell(ui, &block_id, 70.0, false);
                    row_cell(ui, &program, 130.0, false);
                    row_cell(ui, &account_label, 78.0, false);
                    row_cell(ui, &words_label, 84.0, false);
                });
            } else {
                ui.horizontal(|ui| {
                    row_cell(ui, &hash_label, 150.0, true);
                    row_cell(ui, &block_id, 70.0, false);
                    row_cell(ui, &program, 130.0, false);
                    row_cell(ui, &account_label, 78.0, false);
                    row_cell(ui, &words_label, 84.0, false);
                    status_chip(ui, kind);
                    let spacer = (ui.available_width() - 44.0).max(0.0);
                    ui.add_space(spacer);
                    dashboard_row_action(ui);
                });
            }
        });
    let response = ui
        .interact(inner.response.rect, id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            true,
            format!("Open transaction {}", short_token(hash)),
        )
    });
    focus_outline(ui, &response);
    response.clicked()
}

fn dashboard_empty(ui: &mut egui::Ui, text: &str) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 120.0),
        egui::Layout::centered_and_justified(egui::Direction::TopDown),
        |ui| {
            ui.label(egui::RichText::new(text).size(14.0).color(TEXT_MUTED));
        },
    );
}

fn dashboard_row_action(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(INPUT)
        .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Open").size(12.0).strong().color(TEXT));
        });
}

fn dashboard_warning_row(ui: &mut egui::Ui, label: &str, value: &str) {
    let summary = short_inline(value, 96);
    ui.horizontal_top(|ui| {
        ui.add_sized(
            [72.0, 20.0],
            egui::Label::new(
                egui::RichText::new(label)
                    .size(12.0)
                    .strong()
                    .color(TEXT_MUTED),
            ),
        );
        ui.add(egui::Label::new(egui::RichText::new(summary).size(12.0).color(TEXT)).truncate())
            .on_hover_text(value);
    });
}

fn short_inline(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let prefix = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

fn compact_pair_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [100.0, 20.0],
            egui::Label::new(
                egui::RichText::new(label)
                    .size(12.0)
                    .strong()
                    .color(TEXT_MUTED),
            ),
        );
        ui.add(egui::Label::new(egui::RichText::new(value).size(12.0).color(TEXT)).wrap());
    });
}

fn compact_pair_row_linked(
    ui: &mut egui::Ui,
    label: &str,
    lookup_label: &str,
    value: &str,
    lookup: &mut Option<LookupTarget>,
) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [100.0, 20.0],
            egui::Label::new(
                egui::RichText::new(label)
                    .size(12.0)
                    .strong()
                    .color(TEXT_MUTED),
            ),
        );
        if let Some(target) = lookup_target_for_field(lookup_label, value) {
            lookup_link(ui, value, target, lookup);
        } else {
            ui.add(egui::Label::new(egui::RichText::new(value).size(12.0).color(TEXT)).wrap());
        }
    });
}

fn lookup_link(
    ui: &mut egui::Ui,
    value: &str,
    target: LookupTarget,
    lookup: &mut Option<LookupTarget>,
) {
    let response = ui
        .add(egui::Link::new(
            egui::RichText::new(value).size(12.0).monospace(),
        ))
        .on_hover_text(lookup_target_tooltip(&target));
    if response.clicked() && lookup.is_none() {
        *lookup = Some(target);
    }
}

fn row_cell(ui: &mut egui::Ui, value: &str, width: f32, strong: bool) {
    let mut text = egui::RichText::new(value).size(13.0).color(TEXT);
    if strong {
        text = text.strong().monospace();
    }
    ui.add_sized(
        [width, 28.0],
        egui::Label::new(text).truncate().selectable(strong),
    );
}

fn linked_row_cell(
    ui: &mut egui::Ui,
    lookup_label: &str,
    full_value: Option<&str>,
    display_value: &str,
    width: f32,
    lookup: &mut Option<LookupTarget>,
) {
    let target = full_value.and_then(|value| lookup_target_for_field(lookup_label, value));
    if let Some(target) = target {
        ui.allocate_ui_with_layout(
            egui::vec2(width, 28.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| lookup_link(ui, display_value, target, lookup),
        );
    } else {
        row_cell(ui, display_value, width, true);
    }
}

fn idl_registry_row(
    ui: &mut egui::Ui,
    idl: &RegisteredIdl,
    add_actions: impl FnOnce(&mut egui::Ui),
) {
    let (instructions, accounts) = idl_json_counts(&idl.json);
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(&idl.name)
                            .size(15.0)
                            .strong()
                            .color(TEXT),
                    );
                    ui.label(
                        egui::RichText::new(format!("{} bytes", idl.json.len()))
                            .size(12.0)
                            .color(TEXT_MUTED),
                    );
                    if let Some(program_id) = &idl.program_id {
                        ui.label(
                            egui::RichText::new(program_id)
                                .size(12.0)
                                .monospace()
                                .color(TEXT_MUTED),
                        );
                    }
                    ui.horizontal_wrapped(|ui| {
                        status_chip(ui, &format!("{instructions} instructions"));
                        status_chip(ui, &format!("{accounts} accounts"));
                    });
                });
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    add_actions,
                );
            });
        });
}

fn idl_json_counts(json: &str) -> (usize, usize) {
    let Ok(value) = serde_json::from_str::<Value>(json) else {
        return (0, 0);
    };
    (
        value
            .get("instructions")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        value
            .get("accounts")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
    )
}

fn idl_account_names(json: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    value
        .get("accounts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|account| value_str(account, "name").map(ToOwned::to_owned))
        .collect()
}

fn program_id_row(ui: &mut egui::Ui, program: &Value, lookup: &mut Option<LookupTarget>) {
    let label = value_str(program, "label").unwrap_or("Program");
    let base58 = value_str(program, "base58").unwrap_or("-");
    let hex = value_str(program, "hex").unwrap_or("-");

    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(label).size(15.0).strong().color(TEXT));
            ui.add_space(8.0);
            compact_pair_row_linked(ui, "Base58", "program_id_base58", base58, lookup);
            compact_pair_row(ui, "Hex", hex);
        });
}

fn render_detail_value(
    ui: &mut egui::Ui,
    value: &Value,
    account_output_tab: &mut AccountOutputTab,
) -> Option<LookupTarget> {
    let mut lookup = None;
    if value.is_null() {
        render_empty_result(ui, "No result returned");
    } else if render_known_detail(ui, value, &mut lookup, account_output_tab) {
        if !is_account_output_result(value) {
            ui.add_space(10.0);
            egui::CollapsingHeader::new("All fields")
                .id_salt("detail-all-fields")
                .default_open(false)
                .show(ui, |ui| render_value(ui, value, 0, &mut lookup, None));
        }
    } else {
        render_structured_result(ui, value, &mut lookup);
    }
    lookup
}

fn render_known_detail(
    ui: &mut egui::Ui,
    value: &Value,
    lookup: &mut Option<LookupTarget>,
    account_output_tab: &mut AccountOutputTab,
) -> bool {
    if value.get("steps").is_none()
        && let Some(inspection) = value.get("inspection")
    {
        render_transaction_inspection_detail(ui, inspection, lookup);
        if let Some(decoded) = value
            .get("decoded_instruction")
            .filter(|decoded| !decoded.is_null())
        {
            ui.add_space(12.0);
            render_decode_detail(ui, "Decoded instruction", decoded, lookup);
        }
        return true;
    }
    if let (Some(account), Some(decode)) = (value.get("account"), account_decode_output(value)) {
        render_account_output_tabs(ui, account, Some(decode), value, account_output_tab, lookup);
        return true;
    }
    if value.get("block_id").is_some() && value.get("transactions").is_some() {
        render_block_detail(ui, value, lookup);
        return true;
    }
    if value.get("hash").is_some() && value.get("sections").is_some() {
        render_transaction_inspection_detail(ui, value, lookup);
        return true;
    }
    if value.get("hash").is_some() && value.get("steps").is_some() {
        render_trace_detail(ui, value, lookup);
        return true;
    }
    if value.get("hash").is_some() && value.get("kind").is_some() {
        render_transaction_summary_detail(ui, value, lookup);
        return true;
    }
    if value.get("account_id").is_some() && value.get("account").is_some() {
        render_account_output_tabs(ui, value, None, value, account_output_tab, lookup);
        return true;
    }
    if value.get("endpoint").is_some() && value.get("method").is_some() {
        render_rpc_detail(ui, value, lookup);
        return true;
    }
    if (value.get("network_profile").is_some() || value.get("profile").is_some())
        && value.get("sequencer_endpoint").is_some()
        && value.get("indexer_endpoint").is_some()
    {
        render_network_endpoints_detail(ui, value);
        return true;
    }
    if value.get("program_id_base58").is_some() && value.get("deployment_tx_hash").is_some() {
        render_program_file_detail(ui, value, lookup);
        return true;
    }
    if value.get("instruction").is_some() && value.get("variant_index").is_some() {
        render_decode_detail(ui, "Instruction", value, lookup);
        return true;
    }
    if value.get("event").is_some() && value.get("decoded").is_some() {
        render_decode_detail(ui, "Event", value, lookup);
        return true;
    }
    false
}

fn render_account_output_tabs(
    ui: &mut egui::Ui,
    account: &Value,
    decode: Option<&Value>,
    raw: &Value,
    account_output_tab: &mut AccountOutputTab,
    lookup: &mut Option<LookupTarget>,
) {
    if decode.is_none() && *account_output_tab == AccountOutputTab::Decoded {
        *account_output_tab = AccountOutputTab::Detail;
    }
    tab_bar(ui, account_output_tab, &AccountOutputTab::ALL);
    if decode.is_none() && *account_output_tab == AccountOutputTab::Decoded {
        *account_output_tab = AccountOutputTab::Detail;
    }
    ui.add_space(12.0);
    match (*account_output_tab, decode) {
        (AccountOutputTab::Decoded, Some(decode)) => {
            render_decode_detail(ui, "Decoded account", decode, lookup);
        }
        (AccountOutputTab::Decoded, None) | (AccountOutputTab::Detail, _) => {
            render_account_detail(
                ui,
                account,
                decode.and_then(account_definition_type_from_decode),
                lookup,
            );
        }
        (AccountOutputTab::Raw, _) => {
            detail_title_row(ui, "Raw account", "json");
            render_structured_value(ui, raw, lookup, None);
        }
    }
}

fn is_account_output_result(value: &Value) -> bool {
    is_decoded_account_output_result(value)
        || (value.get("account_id").is_some() && value.get("account").is_some())
}

fn is_decoded_account_output_result(value: &Value) -> bool {
    value.get("account").is_some() && account_decode_output(value).is_some()
}

fn account_decode_output(value: &Value) -> Option<&Value> {
    value.get("decode").filter(|decode| !decode.is_null())
}

fn default_account_output_tab(value: &Value) -> Option<AccountOutputTab> {
    if is_decoded_account_output_result(value) {
        Some(AccountOutputTab::Decoded)
    } else if is_account_output_result(value) {
        Some(AccountOutputTab::Detail)
    } else {
        None
    }
}

fn render_block_detail(ui: &mut egui::Ui, value: &Value, lookup: &mut Option<LookupTarget>) {
    detail_title_row(
        ui,
        "Block",
        value_str(value, "bedrock_status").unwrap_or("unknown"),
    );
    detail_stat_grid_linked(
        ui,
        &[
            (
                "Block ID",
                value_u64(value, "block_id").map_or("-".to_owned(), |v| format!("#{v}")),
            ),
            (
                "Status",
                value_str(value, "bedrock_status").unwrap_or("-").to_owned(),
            ),
            (
                "Transactions",
                value_usize(value, "tx_count").map_or("-".to_owned(), |v| v.to_string()),
            ),
            (
                "Time",
                value_u64(value, "timestamp").map_or("-".to_owned(), timestamp_label),
            ),
        ],
        lookup,
    );
    if let Some(warning) = value_str(value, "decode_warning") {
        ui.add_space(12.0);
        warning_panel(ui, "Decode warning", warning);
    }
    if let Some(transactions) = value.get("transactions").and_then(Value::as_array) {
        ui.add_space(12.0);
        detail_heading(ui, "Transactions");
        if transactions.is_empty() {
            detail_empty(ui, "No transactions in this block");
        } else {
            for transaction in transactions {
                transaction_detail_row(ui, transaction, value_u64(value, "block_id"), lookup);
                ui.add_space(10.0);
            }
        }
    }
}

fn render_transaction_summary_detail(
    ui: &mut egui::Ui,
    value: &Value,
    lookup: &mut Option<LookupTarget>,
) {
    render_transaction_overview(ui, value, lookup);
    ui.add_space(12.0);
    render_transaction_summary_sections(ui, value, lookup);
}

fn render_transaction_overview(
    ui: &mut egui::Ui,
    value: &Value,
    lookup: &mut Option<LookupTarget>,
) {
    detail_title_row(
        ui,
        "Transaction",
        value_str(value, "kind").unwrap_or("unknown"),
    );
    let bytecode = value_usize(value, "bytecode_len")
        .map(|value| format!("{value} bytes"))
        .unwrap_or_else(|| "-".to_owned());
    detail_stat_grid_linked(
        ui,
        &[
            ("Hash", value_str(value, "hash").unwrap_or("-").to_owned()),
            ("Kind", value_str(value, "kind").unwrap_or("-").to_owned()),
            (
                "Program",
                value_str(value, "program_id_hex")
                    .map(short_token)
                    .unwrap_or_else(|| "-".to_owned()),
            ),
            (
                "Accounts",
                value
                    .get("account_ids")
                    .and_then(Value::as_array)
                    .map_or("-".to_owned(), |items| items.len().to_string()),
            ),
            (
                "Nonces",
                value
                    .get("nonces")
                    .and_then(Value::as_array)
                    .map_or("-".to_owned(), |items| items.len().to_string()),
            ),
            (
                "Instruction words",
                value
                    .get("instruction_data")
                    .and_then(Value::as_array)
                    .map_or("-".to_owned(), |items| items.len().to_string()),
            ),
            ("Bytecode", bytecode),
        ],
        lookup,
    );
    ui.add_space(10.0);
    if let Some(hash) = value_str(value, "hash") {
        detail_token_row_linked(ui, "Hash", hash, lookup);
    }
    if let Some(program_id) = value_str(value, "program_id_hex") {
        detail_token_row_linked(ui, "Program ID", program_id, lookup);
    }
    render_validation_summary(ui, value);
}

fn render_transaction_inspection_detail(
    ui: &mut egui::Ui,
    value: &Value,
    lookup: &mut Option<LookupTarget>,
) {
    let summary = value.get("raw_summary").unwrap_or(value);
    render_transaction_overview(ui, summary, lookup);
    if let Some(sections) = value.get("sections").and_then(Value::as_array) {
        ui.add_space(12.0);
        for section in sections {
            if value_str(section, "title").is_some_and(|title| title == "Summary") {
                continue;
            }
            render_inspection_section(ui, section, lookup);
            ui.add_space(10.0);
        }
    }
}

fn render_trace_detail(ui: &mut egui::Ui, value: &Value, lookup: &mut Option<LookupTarget>) {
    let summary = value
        .get("inspection")
        .and_then(|inspection| inspection.get("raw_summary"))
        .unwrap_or(value);
    render_transaction_overview(ui, summary, lookup);
    ui.add_space(12.0);
    let trace_source = value_str(value, "source")
        .map(short_inline_source)
        .unwrap_or_else(|| "sequencer".to_owned());
    detail_title_row(ui, "Trace", &trace_source);
    detail_stat_grid(
        ui,
        &[
            (
                "Source",
                value_str(value, "source")
                    .map(short_inline_source)
                    .unwrap_or_else(|| "-".to_owned()),
            ),
            (
                "Steps",
                value
                    .get("steps")
                    .and_then(Value::as_array)
                    .map_or("-".to_owned(), |items| items.len().to_string()),
            ),
            (
                "Capabilities",
                value
                    .get("capabilities")
                    .and_then(Value::as_array)
                    .map_or("-".to_owned(), |items| items.len().to_string()),
            ),
            (
                "Limitations",
                value
                    .get("limitations")
                    .and_then(Value::as_array)
                    .map_or("-".to_owned(), |items| items.len().to_string()),
            ),
        ],
    );
    render_trace_notes(ui, value, "Capabilities", "capabilities");
    render_trace_notes(ui, value, "Limitations", "limitations");
    if let Some(steps) = value.get("steps").and_then(Value::as_array) {
        ui.add_space(12.0);
        detail_heading(ui, "Timeline");
        for step in steps {
            trace_step_row(ui, step, lookup);
            ui.add_space(10.0);
        }
    }
    if let Some(decoded) = value
        .get("decoded_instruction")
        .filter(|decoded| !decoded.is_null())
    {
        ui.add_space(12.0);
        render_decode_detail(ui, "Decoded instruction", decoded, lookup);
    }
    if let Some(inspection) = value.get("inspection") {
        ui.add_space(12.0);
        egui::CollapsingHeader::new("Inspection sections")
            .id_salt("trace-inspection-sections")
            .default_open(false)
            .show(ui, |ui| {
                render_transaction_inspection_detail(ui, inspection, lookup)
            });
    }
}

fn render_account_detail(
    ui: &mut egui::Ui,
    value: &Value,
    definition_type: Option<&str>,
    lookup: &mut Option<LookupTarget>,
) {
    detail_title_row(ui, "Account", "lookup");
    let account = value.get("account");
    let data_hex = value_str(value, "data_hex");
    let data_len = data_hex
        .map(|text| format!("{} bytes", text.len() / 2))
        .or_else(|| account.and_then(account_data_len_label))
        .unwrap_or_else(|| "-".to_owned());
    let owner = account
        .and_then(account_owner_label)
        .unwrap_or_else(|| "-".to_owned());
    let balance = account
        .and_then(|account| account.get("balance"))
        .map(value_text)
        .unwrap_or_else(|| "-".to_owned());
    let nonce = account
        .and_then(|account| account.get("nonce"))
        .map(value_text)
        .unwrap_or_else(|| "-".to_owned());
    let mut stats = vec![
        (
            "Account".to_owned(),
            value_str(value, "account_id").unwrap_or("-").to_owned(),
        ),
        ("Owner".to_owned(), owner),
        ("Balance".to_owned(), balance),
        ("Nonce".to_owned(), nonce),
        ("Data".to_owned(), data_len),
    ];
    if let Some(definition_type) = definition_type.or_else(|| value_str(value, "definition_type")) {
        stats.push(("DefinitionType".to_owned(), definition_type.to_owned()));
    }
    if let Some(transactions) = value.get("related_transactions").and_then(Value::as_array) {
        stats.push(("Transactions".to_owned(), transactions.len().to_string()));
    }
    detail_stat_grid_dynamic_linked(ui, &stats, lookup);
    if let Some(account_id) = value_str(value, "account_id") {
        detail_token_row_linked(ui, "Account ID", account_id, lookup);
    }
    if let Some(data_hex) = data_hex {
        detail_token_row(ui, "Data hex", data_hex);
    }
    if let Some(account) = account {
        ui.add_space(12.0);
        result_section(ui, "Account fields", |ui| {
            render_structured_value(ui, account, lookup, None)
        });
    }
    if let Some(error) = value_str(value, "related_transactions_error") {
        ui.add_space(12.0);
        warning_panel(ui, "Related transactions unavailable", error);
    }
    if let Some(transactions) = value.get("related_transactions").and_then(Value::as_array) {
        ui.add_space(12.0);
        detail_heading(ui, "Related transactions");
        if transactions.is_empty() {
            detail_empty(ui, "No related transactions found");
        } else {
            for transaction in transactions {
                transaction_detail_row(ui, transaction, value_u64(transaction, "block_id"), lookup);
                ui.add_space(10.0);
            }
        }
    }
}

fn render_rpc_detail(ui: &mut egui::Ui, value: &Value, lookup: &mut Option<LookupTarget>) {
    detail_title_row(
        ui,
        "RPC response",
        value_str(value, "method").unwrap_or("rpc"),
    );
    let response = value.get("response");
    let status = response
        .map(rpc_status_label)
        .unwrap_or_else(|| "-".to_owned());
    let result_kind = response
        .and_then(|response| response.get("result"))
        .map(value_kind_label)
        .unwrap_or_else(|| "-".to_owned());
    let response_id = response
        .and_then(|response| response.get("id"))
        .map(value_text)
        .unwrap_or_else(|| "-".to_owned());
    detail_stat_grid(
        ui,
        &[
            (
                "Endpoint",
                value_str(value, "endpoint").unwrap_or("-").to_owned(),
            ),
            (
                "Method",
                value_str(value, "method").unwrap_or("-").to_owned(),
            ),
            ("Status", status),
            ("ID", response_id),
            ("Result", result_kind),
        ],
    );
    if let Some(endpoint) = value_str(value, "endpoint") {
        detail_token_row(ui, "Endpoint", endpoint);
    }
    if let Some(response) = response {
        if let Some(error) = response.get("error") {
            ui.add_space(12.0);
            error_panel(ui, &value_text(error));
        }
        ui.add_space(12.0);
        if let Some(result) = response.get("result") {
            let mut account_output_tab =
                default_account_output_tab(result).unwrap_or(AccountOutputTab::Detail);
            result_section(ui, "Result", |ui| {
                render_rpc_payload(ui, result, lookup, &mut account_output_tab)
            });
        } else {
            detail_empty(ui, "Response has no result field");
        }
    }
}

fn render_program_file_detail(ui: &mut egui::Ui, value: &Value, lookup: &mut Option<LookupTarget>) {
    detail_title_row(ui, "Program binary", "local file");
    detail_stat_grid_linked(
        ui,
        &[
            ("Path", value_str(value, "path").unwrap_or("-").to_owned()),
            (
                "Bytecode",
                value_usize(value, "bytecode_len").map_or("-".to_owned(), |v| v.to_string()),
            ),
            (
                "Program ID",
                value_str(value, "program_id_base58")
                    .unwrap_or("-")
                    .to_owned(),
            ),
            (
                "Deploy tx",
                value_str(value, "deployment_tx_hash")
                    .map(short_token)
                    .unwrap_or_else(|| "-".to_owned()),
            ),
        ],
        lookup,
    );
    if let Some(path) = value_str(value, "path") {
        detail_token_row(ui, "Path", path);
    }
    if let Some(program_id) = value_str(value, "program_id_base58") {
        detail_token_row_linked(ui, "Program ID", program_id, lookup);
    }
    if let Some(program_id) = value_str(value, "program_id_hex") {
        detail_token_row_linked(ui, "Program hex", program_id, lookup);
    }
    if let Some(hash) = value_str(value, "deployment_tx_hash") {
        detail_token_row_linked(ui, "Deploy tx", hash, lookup);
    }
}

fn render_decode_detail(
    ui: &mut egui::Ui,
    title: &str,
    value: &Value,
    lookup: &mut Option<LookupTarget>,
) {
    detail_title_row(
        ui,
        title,
        value_str(value, "instruction")
            .or_else(|| value_str(value, "event"))
            .or_else(|| value_str(value, "account_type"))
            .unwrap_or("decoded"),
    );
    if value.get("instruction").is_some() && value.get("variant_index").is_some() {
        render_instruction_decode_detail(ui, value, lookup);
    } else if let Some(rows) = value.get("rows").and_then(Value::as_array) {
        let type_stat_label = if value.get("account_type").is_some() {
            "DefinitionType"
        } else {
            "Type"
        };
        let type_label = value_str(value, "account_type")
            .or_else(|| value_str(value, "event"))
            .unwrap_or(title)
            .to_owned();
        let mut stats = vec![
            (type_stat_label.to_owned(), type_label),
            (
                "Bytes".to_owned(),
                match (
                    value_usize(value, "consumed_bytes"),
                    value_usize(value, "total_bytes"),
                ) {
                    (Some(consumed), Some(total)) => format!("{consumed}/{total}"),
                    _ => "-".to_owned(),
                },
            ),
        ];
        if let Some(remaining) = value_usize(value, "remaining_bytes") {
            stats.push(("Remaining".to_owned(), format!("{remaining} bytes")));
        }
        detail_stat_grid_dynamic(ui, &stats);
        ui.add_space(10.0);
        for row in rows {
            decoded_field_row(ui, row, lookup);
            ui.add_space(8.0);
        }
        if let Some(decoded) = value.get("decoded") {
            ui.add_space(10.0);
            result_section(ui, "Decoded value", |ui| {
                render_structured_value(ui, decoded, lookup, None);
            });
        }
    } else {
        render_structured_value(ui, value.get("decoded").unwrap_or(value), lookup, None);
    }
}

fn detail_heading(ui: &mut egui::Ui, title: &str) {
    ui.label(egui::RichText::new(title).size(15.0).strong().color(TEXT));
    ui.add_space(8.0);
}

fn detail_title_row(ui: &mut egui::Ui, title: &str, status: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(title).size(16.0).strong().color(TEXT));
        if !status.is_empty() && status != "-" {
            status_chip(ui, status);
        }
    });
    ui.add_space(10.0);
}

fn detail_stat_grid(ui: &mut egui::Ui, stats: &[(&str, String)]) {
    let stats = stats
        .iter()
        .map(|(label, value)| ((*label).to_owned(), value.clone()))
        .collect::<Vec<_>>();
    detail_stat_grid_dynamic(ui, &stats);
}

fn detail_stat_grid_linked(
    ui: &mut egui::Ui,
    stats: &[(&str, String)],
    lookup: &mut Option<LookupTarget>,
) {
    let stats = stats
        .iter()
        .map(|(label, value)| ((*label).to_owned(), value.clone()))
        .collect::<Vec<_>>();
    detail_stat_grid_dynamic_linked(ui, &stats, lookup);
}

fn detail_stat_grid_dynamic(ui: &mut egui::Ui, stats: &[(String, String)]) {
    detail_stat_grid_dynamic_inner(ui, stats, None);
}

fn detail_stat_grid_dynamic_linked(
    ui: &mut egui::Ui,
    stats: &[(String, String)],
    lookup: &mut Option<LookupTarget>,
) {
    detail_stat_grid_dynamic_inner(ui, stats, Some(lookup));
}

fn detail_stat_grid_dynamic_inner(
    ui: &mut egui::Ui,
    stats: &[(String, String)],
    mut lookup: Option<&mut Option<LookupTarget>>,
) {
    if stats.is_empty() {
        detail_empty(ui, "No values");
        return;
    }
    let columns = if ui.available_width() >= 720.0 {
        3
    } else if ui.available_width() >= 460.0 {
        2
    } else {
        1
    };
    for row in stats.chunks(columns) {
        ui.columns(row.len(), |columns| {
            for (column, (label, value)) in columns.iter_mut().zip(row.iter()) {
                egui::Frame::new()
                    .fill(INPUT)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(10))
                    .show(column, |ui| {
                        ui.set_min_height(54.0);
                        ui.label(
                            egui::RichText::new(label.as_str())
                                .size(11.0)
                                .strong()
                                .color(TEXT_MUTED),
                        );
                        ui.add_space(4.0);
                        if let Some(target) = lookup_target_for_field(label, value)
                            && let Some(lookup) = lookup.as_deref_mut()
                        {
                            lookup_link(ui, value, target, lookup);
                        } else {
                            ui.add(
                                egui::Label::new(egui::RichText::new(value).size(13.0).color(TEXT))
                                    .truncate(),
                            )
                            .on_hover_text(value);
                        }
                    });
            }
        });
        ui.add_space(8.0);
    }
}

fn detail_token_row(ui: &mut egui::Ui, label: &str, value: &str) {
    egui::Frame::new()
        .fill(INPUT)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.add_sized(
                    [96.0, 20.0],
                    egui::Label::new(
                        egui::RichText::new(label)
                            .size(12.0)
                            .strong()
                            .color(TEXT_MUTED),
                    ),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(value)
                            .size(12.0)
                            .monospace()
                            .color(TEXT),
                    )
                    .wrap()
                    .selectable(true),
                );
            });
        });
}

fn detail_token_row_linked(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    lookup: &mut Option<LookupTarget>,
) {
    egui::Frame::new()
        .fill(INPUT)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.add_sized(
                    [96.0, 20.0],
                    egui::Label::new(
                        egui::RichText::new(label)
                            .size(12.0)
                            .strong()
                            .color(TEXT_MUTED),
                    ),
                );
                if let Some(target) = lookup_target_for_field(label, value) {
                    lookup_link(ui, value, target, lookup);
                } else {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(value)
                                .size(12.0)
                                .monospace()
                                .color(TEXT),
                        )
                        .wrap()
                        .selectable(true),
                    );
                }
            });
        });
}

fn detail_empty(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(INPUT)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(text).size(13.0).color(TEXT_MUTED));
        });
}

fn render_empty_result(ui: &mut egui::Ui, text: &str) {
    detail_title_row(ui, "Result", "empty");
    detail_empty(ui, text);
}

fn render_network_endpoints_detail(ui: &mut egui::Ui, value: &Value) {
    detail_title_row(ui, "Network", "active");
    let profile = value_str(value, "profile")
        .or_else(|| value_str(value, "network_profile"))
        .unwrap_or("-");
    detail_stat_grid(
        ui,
        &[
            ("Profile", profile.to_owned()),
            (
                "Sequencer",
                value_str(value, "sequencer_endpoint")
                    .map(compact_endpoint)
                    .unwrap_or_else(|| "-".to_owned()),
            ),
            (
                "Indexer",
                value_str(value, "indexer_endpoint")
                    .map(compact_endpoint)
                    .unwrap_or_else(|| "-".to_owned()),
            ),
        ],
    );
    if let Some(endpoint) = value_str(value, "sequencer_endpoint") {
        detail_token_row(ui, "Sequencer", endpoint);
    }
    if let Some(endpoint) = value_str(value, "indexer_endpoint") {
        detail_token_row(ui, "Indexer", endpoint);
    }
}

fn transaction_detail_row(
    ui: &mut egui::Ui,
    transaction: &Value,
    block_id: Option<u64>,
    lookup: &mut Option<LookupTarget>,
) {
    let hash = value_str(transaction, "hash")
        .map(short_token)
        .unwrap_or_else(|| "-".to_owned());
    let kind = value_str(transaction, "kind").unwrap_or("-");
    let block_lookup = block_id.map(|value| value.to_string());
    let block = block_id.map_or("-".to_owned(), |value| format!("#{value}"));
    let program = value_str(transaction, "program_id_hex")
        .map(short_token)
        .unwrap_or_else(|| "-".to_owned());
    let accounts = transaction
        .get("account_ids")
        .and_then(Value::as_array)
        .map_or("-".to_owned(), |items| items.len().to_string());
    let nonces = transaction
        .get("nonces")
        .and_then(Value::as_array)
        .map_or("-".to_owned(), |items| items.len().to_string());
    let words = transaction
        .get("instruction_data")
        .and_then(Value::as_array)
        .map_or("-".to_owned(), |items| items.len().to_string());
    let bytecode = value_usize(transaction, "bytecode_len")
        .map(|value| format!("{value} bytes"))
        .unwrap_or_else(|| "-".to_owned());
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if ui.available_width() < 620.0 {
                ui.horizontal_wrapped(|ui| {
                    linked_row_cell(
                        ui,
                        "hash",
                        value_str(transaction, "hash"),
                        &hash,
                        160.0,
                        lookup,
                    );
                    status_chip(ui, kind);
                    linked_row_cell(
                        ui,
                        "block_id",
                        block_lookup.as_deref(),
                        &block,
                        70.0,
                        lookup,
                    );
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    linked_row_cell(
                        ui,
                        "program_id_hex",
                        value_str(transaction, "program_id_hex"),
                        &program,
                        140.0,
                        lookup,
                    );
                    row_cell(ui, &format!("{accounts} accts"), 84.0, false);
                    row_cell(ui, &format!("{words} words"), 92.0, false);
                    row_cell(ui, &bytecode, 110.0, false);
                });
            } else {
                ui.horizontal(|ui| {
                    linked_row_cell(
                        ui,
                        "hash",
                        value_str(transaction, "hash"),
                        &hash,
                        160.0,
                        lookup,
                    );
                    linked_row_cell(
                        ui,
                        "block_id",
                        block_lookup.as_deref(),
                        &block,
                        70.0,
                        lookup,
                    );
                    linked_row_cell(
                        ui,
                        "program_id_hex",
                        value_str(transaction, "program_id_hex"),
                        &program,
                        140.0,
                        lookup,
                    );
                    row_cell(ui, &format!("{accounts} accts"), 84.0, false);
                    row_cell(ui, &format!("{nonces} nonces"), 94.0, false);
                    row_cell(ui, &format!("{words} words"), 92.0, false);
                    row_cell(ui, &bytecode, 110.0, false);
                    let spacer = (ui.available_width() - 96.0).max(0.0);
                    ui.add_space(spacer);
                    status_chip(ui, kind);
                });
            }
        });
}

fn render_transaction_summary_sections(
    ui: &mut egui::Ui,
    value: &Value,
    lookup: &mut Option<LookupTarget>,
) {
    if let Some(accounts) = value
        .get("account_ids")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
    {
        render_string_list_section(ui, "Accounts", "account", accounts, lookup);
        ui.add_space(10.0);
    }
    if let Some(nonces) = value
        .get("nonces")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
    {
        render_string_list_section(ui, "Nonces", "nonce", nonces, lookup);
        ui.add_space(10.0);
    }
    if let Some(words) = value
        .get("instruction_data")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
    {
        render_instruction_words_section(ui, words);
        ui.add_space(10.0);
    }
    if let Some(bytecode_len) = value_usize(value, "bytecode_len") {
        result_section(ui, "Deployment payload", |ui| {
            compact_pair_row(ui, "Bytecode", &format!("{bytecode_len} bytes"));
        });
    }
}

fn render_string_list_section(
    ui: &mut egui::Ui,
    title: &str,
    item_label: &str,
    items: &[Value],
    lookup: &mut Option<LookupTarget>,
) {
    result_section(ui, title, |ui| {
        if items.is_empty() {
            ui.label(egui::RichText::new("No entries").color(TEXT_MUTED));
        } else {
            for (index, item) in items.iter().enumerate() {
                let value = item
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value_text(item));
                let label = format!("{item_label}[{index}]");
                compact_pair_row_linked(ui, &label, &label, &value, lookup);
            }
        }
    });
}

fn render_instruction_words_section(ui: &mut egui::Ui, words: &[Value]) {
    result_section(ui, "Instruction words", |ui| {
        if words.is_empty() {
            ui.label(egui::RichText::new("No instruction words").color(TEXT_MUTED));
        } else {
            for (index, word) in words.iter().enumerate() {
                let decimal = word
                    .as_u64()
                    .map_or_else(|| value_text(word), |word| word.to_string());
                let hex = word
                    .as_u64()
                    .map_or_else(|| "-".to_owned(), |word| format!("0x{word:08x}"));
                compact_pair_row(ui, &format!("word[{index}]"), &format!("{decimal} / {hex}"));
            }
        }
    });
}

fn render_validation_summary(ui: &mut egui::Ui, value: &Value) {
    let mut rows = Vec::new();
    if let Some(valid) = value.get("raw_signature_valid").and_then(Value::as_bool) {
        rows.push(("Raw signature", validity_label(valid)));
    }
    if let Some(prehash) = value_str(value, "message_prehash") {
        rows.push(("Message prehash", short_token(prehash)));
    }
    if let Some(valid) = value
        .get("prehash_signature_valid")
        .and_then(Value::as_bool)
    {
        rows.push(("Prehash signature", validity_label(valid)));
    }
    if rows.is_empty() {
        return;
    }
    ui.add_space(10.0);
    result_section(ui, "Validation", |ui| {
        for (label, value) in rows {
            compact_pair_row(ui, label, &value);
        }
    });
}

fn render_inspection_section(
    ui: &mut egui::Ui,
    section: &Value,
    lookup: &mut Option<LookupTarget>,
) {
    let title = value_str(section, "title").unwrap_or("Section");
    result_section(ui, title, |ui| {
        let Some(rows) = section.get("rows").and_then(Value::as_array) else {
            ui.label(egui::RichText::new("No rows").color(TEXT_MUTED));
            return;
        };
        if rows.is_empty() {
            ui.label(egui::RichText::new("No rows").color(TEXT_MUTED));
            return;
        }
        for row in rows {
            inspection_row(ui, row, lookup);
            ui.add_space(8.0);
        }
    });
}

fn inspection_row(ui: &mut egui::Ui, row: &Value, lookup: &mut Option<LookupTarget>) {
    let label = inspection_row_label(row);
    let value = value_str(row, "value").unwrap_or("-");
    egui::Frame::new()
        .fill(INPUT)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            compact_pair_row_linked(ui, &label, &label, value, lookup);
            let mut alternates = Vec::new();
            if let Some(decimal) = value_str(row, "decimal").filter(|decimal| *decimal != value) {
                alternates.push(("Decimal", "Decimal", decimal));
            }
            if let Some(hex) = value_str(row, "hex").filter(|hex| *hex != value) {
                alternates.push(("Hex", &label, hex));
            }
            if let Some(base58) = value_str(row, "base58").filter(|base58| *base58 != value) {
                alternates.push(("Base58", &label, base58));
            }
            for (label, lookup_label, value) in alternates {
                compact_pair_row_linked(ui, label, lookup_label, value, lookup);
            }
        });
}

fn inspection_row_label(row: &Value) -> String {
    let label = value_str(row, "label").unwrap_or("row");
    row.get("index").and_then(Value::as_u64).map_or_else(
        || format_label(label),
        |index| format!("{}[{index}]", format_label(label)),
    )
}

fn validity_label(valid: bool) -> String {
    if valid {
        "valid".to_owned()
    } else {
        "invalid".to_owned()
    }
}

fn short_inline_source(value: &str) -> String {
    short_inline(value, 42)
}

fn render_trace_notes(ui: &mut egui::Ui, value: &Value, title: &str, key: &str) {
    let Some(items) = value.get(key).and_then(Value::as_array) else {
        return;
    };
    if items.is_empty() {
        return;
    }
    ui.add_space(12.0);
    result_section(ui, title, |ui| {
        for item in items {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!("- {}", value_text(item)))
                        .size(12.0)
                        .color(TEXT),
                )
                .wrap(),
            );
        }
    });
}

fn trace_step_row(ui: &mut egui::Ui, step: &Value, lookup: &mut Option<LookupTarget>) {
    let index = value_usize(step, "index").map_or("-".to_owned(), |index| format!("#{index}"));
    let phase = value_str(step, "phase").unwrap_or("-");
    let label = value_str(step, "label").unwrap_or("Step");
    let status = value_str(step, "status").unwrap_or("observed");
    let severity = value_str(step, "severity");
    let stroke = if severity.is_some() {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(179, 126, 52))
    } else {
        egui::Stroke::new(1.0, BORDER_STRONG)
    };
    egui::Frame::new()
        .fill(PANEL)
        .stroke(stroke)
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                row_cell(ui, &index, 54.0, true);
                status_chip(ui, phase);
                status_pill(ui, status);
                if let Some(severity) = severity {
                    tag_pill(ui, severity);
                }
            });
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format_label(label))
                    .size(14.0)
                    .strong()
                    .color(TEXT),
            );
            if let Some(details) = step.get("details").and_then(Value::as_array) {
                ui.add_space(8.0);
                for detail in details {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("- {}", value_text(detail)))
                                .size(12.0)
                                .color(TEXT_MUTED),
                        )
                        .wrap(),
                    );
                }
            }
            if let Some(refs) = step.get("refs").and_then(Value::as_object) {
                ui.add_space(8.0);
                for (key, value) in refs.iter().filter(|(_, value)| !value.is_null()) {
                    compact_pair_row_linked(
                        ui,
                        &format_label(key),
                        key,
                        &value_text(value),
                        lookup,
                    );
                }
            }
        });
}

fn render_instruction_decode_detail(
    ui: &mut egui::Ui,
    value: &Value,
    lookup: &mut Option<LookupTarget>,
) {
    let remaining = value
        .get("remaining_words")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    detail_stat_grid(
        ui,
        &[
            (
                "Instruction",
                value_str(value, "instruction").unwrap_or("-").to_owned(),
            ),
            (
                "Variant",
                value_u64(value, "variant_index").map_or("-".to_owned(), |value| value.to_string()),
            ),
            (
                "IDL",
                value_str(value, "idl_name").unwrap_or("ad hoc").to_owned(),
            ),
            ("Remaining words", remaining.to_string()),
        ],
    );
    if let Some(program_id) = value_str(value, "program_id") {
        detail_token_row_linked(ui, "Program ID", program_id, lookup);
    }
    render_decoded_fields_section(ui, value, "Accounts", "accounts", lookup);
    render_decoded_fields_section(ui, value, "Arguments", "args", lookup);
    if let Some(words) = value.get("remaining_words").and_then(Value::as_array)
        && !words.is_empty()
    {
        ui.add_space(10.0);
        render_instruction_words_section(ui, words);
    }
}

fn render_decoded_fields_section(
    ui: &mut egui::Ui,
    value: &Value,
    title: &str,
    key: &str,
    lookup: &mut Option<LookupTarget>,
) {
    let Some(fields) = value.get(key).and_then(Value::as_array) else {
        return;
    };
    ui.add_space(10.0);
    result_section(ui, title, |ui| {
        if fields.is_empty() {
            ui.label(egui::RichText::new("No fields").color(TEXT_MUTED));
        } else {
            for field in fields {
                decoded_field_row(ui, field, lookup);
                ui.add_space(8.0);
            }
        }
    });
}

fn decoded_field_row(ui: &mut egui::Ui, field: &Value, lookup: &mut Option<LookupTarget>) {
    let label = value_str(field, "path").unwrap_or("-");
    let value = value_str(field, "value").unwrap_or("-");
    egui::Frame::new()
        .fill(INPUT)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            compact_pair_row_linked(ui, label, label, value, lookup);
        });
}

fn render_rpc_payload(
    ui: &mut egui::Ui,
    value: &Value,
    lookup: &mut Option<LookupTarget>,
    account_output_tab: &mut AccountOutputTab,
) {
    if value.is_null() {
        detail_empty(ui, "No result");
    } else if !is_scalar(value) && render_known_detail(ui, value, lookup, account_output_tab) {
        // Already rendered by a domain-specific detail view.
    } else {
        render_structured_value(ui, value, lookup, None);
    }
}

fn render_structured_result(ui: &mut egui::Ui, value: &Value, lookup: &mut Option<LookupTarget>) {
    let kind = value_kind_label(value);
    detail_title_row(ui, "Result", &kind);
    render_structured_value(ui, value, lookup, None);
}

fn render_structured_value(
    ui: &mut egui::Ui,
    value: &Value,
    lookup: &mut Option<LookupTarget>,
    context: Option<&str>,
) {
    match value {
        Value::Null => detail_empty(ui, "No value"),
        Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            let label = context
                .map(format_label)
                .unwrap_or_else(|| "Value".to_owned());
            detail_stat_grid_dynamic_linked(ui, &[(label, value_text(value))], lookup);
        }
        Value::Array(items) => render_structured_array(ui, items, lookup, context),
        Value::Object(object) => render_structured_object(ui, object, lookup),
    }
}

fn render_structured_object(
    ui: &mut egui::Ui,
    object: &serde_json::Map<String, Value>,
    lookup: &mut Option<LookupTarget>,
) {
    if object.is_empty() {
        detail_empty(ui, "No fields");
        return;
    }

    let scalar_rows = object
        .iter()
        .filter(|(_, value)| is_scalar(value))
        .map(|(key, value)| (format_label(key), value_text(value)))
        .collect::<Vec<_>>();
    if !scalar_rows.is_empty() {
        detail_stat_grid_dynamic_linked(ui, &scalar_rows, lookup);
    }

    for (key, value) in object.iter().filter(|(_, value)| !is_scalar(value)) {
        ui.add_space(10.0);
        result_section(ui, &format_label(key), |ui| {
            render_structured_value(ui, value, lookup, Some(key));
        });
    }
}

fn render_structured_array(
    ui: &mut egui::Ui,
    items: &[Value],
    lookup: &mut Option<LookupTarget>,
    context: Option<&str>,
) {
    detail_stat_grid(ui, &[("Items", items.len().to_string())]);
    if items.is_empty() {
        detail_empty(ui, "No items");
        return;
    }

    if items.iter().all(is_scalar) {
        let rows = items
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let label = context.map_or_else(
                    || format!("#{index}"),
                    |context| format!("{}[{index}]", format_label(context)),
                );
                (label, value_text(value))
            })
            .collect::<Vec<_>>();
        detail_stat_grid_dynamic_linked(ui, &rows, lookup);
        return;
    }

    for (index, item) in items.iter().enumerate() {
        let title = item
            .as_object()
            .and_then(|object| {
                object
                    .get("title")
                    .or_else(|| object.get("label"))
                    .or_else(|| object.get("name"))
                    .or_else(|| object.get("hash"))
                    .or_else(|| object.get("block_id"))
            })
            .map(value_text)
            .filter(|value| value != "-")
            .map(|value| short_inline(&value, 48))
            .unwrap_or_else(|| format!("Item {}", index + 1));
        result_section(ui, &title, |ui| {
            render_structured_value(ui, item, lookup, context)
        });
        ui.add_space(10.0);
    }
}

fn account_owner_label(account: &Value) -> Option<String> {
    for key in ["program_owner", "owner", "program_id"] {
        let Some(value) = account.get(key) else {
            continue;
        };
        return Some(match value {
            Value::String(value) => short_token(value),
            Value::Array(items) => format!("{} words", items.len()),
            _ => value_text(value),
        });
    }
    None
}

fn account_data_len_label(account: &Value) -> Option<String> {
    let data = account.get("data")?;
    Some(match data {
        Value::Array(items) => format!("{} bytes", items.len()),
        Value::String(value) => format!("{} base64 chars", value.len()),
        _ => value_summary(data),
    })
}

fn rpc_status_label(response: &Value) -> String {
    if response.get("error").is_some() {
        "error".to_owned()
    } else if response.get("result").is_some() {
        "ok".to_owned()
    } else {
        "unknown".to_owned()
    }
}

fn value_kind_label(value: &Value) -> String {
    match value {
        Value::Null => "empty".to_owned(),
        Value::Bool(_) => "boolean".to_owned(),
        Value::Number(_) => "number".to_owned(),
        Value::String(_) => "text".to_owned(),
        Value::Array(items) => format!("{} items", items.len()),
        Value::Object(object) => format!("{} fields", object.len()),
    }
}

fn render_value(
    ui: &mut egui::Ui,
    value: &Value,
    depth: usize,
    lookup: &mut Option<LookupTarget>,
    context: Option<&str>,
) {
    match value {
        Value::Object(object) => render_object(ui, object, depth, lookup),
        Value::Array(items) => render_array(ui, items, depth, lookup, context),
        _ => {
            value_widget(
                ui,
                context.unwrap_or("value"),
                value,
                value_text(value),
                lookup,
            );
        }
    }
}

fn render_object(
    ui: &mut egui::Ui,
    object: &serde_json::Map<String, Value>,
    depth: usize,
    lookup: &mut Option<LookupTarget>,
) {
    if object.is_empty() {
        ui.label(egui::RichText::new("No fields").color(TEXT_MUTED));
        return;
    }

    let mut has_scalar = false;
    for (key, value) in object.iter().filter(|(_, value)| is_scalar(value)) {
        has_scalar = true;
        kv_row(ui, key, value, lookup);
    }
    if has_scalar {
        ui.add_space(10.0);
    }

    for (key, value) in object.iter().filter(|(_, value)| !is_scalar(value)) {
        result_section(ui, &format_label(key), |ui| {
            if depth >= 6 {
                ui.label(egui::RichText::new(value_summary(value)).color(TEXT_MUTED));
            } else {
                render_value(ui, value, depth + 1, lookup, Some(key));
            }
        });
        ui.add_space(10.0);
    }
}

fn render_array(
    ui: &mut egui::Ui,
    items: &[Value],
    depth: usize,
    lookup: &mut Option<LookupTarget>,
    context: Option<&str>,
) {
    if items.is_empty() {
        ui.label(egui::RichText::new("No items").color(TEXT_MUTED));
        return;
    }

    if items.iter().all(is_scalar) {
        for (index, value) in items.iter().enumerate() {
            let label = context.map_or_else(
                || format!("#{index}"),
                |context| format!("{}[{index}]", format_label(context)),
            );
            kv_row(ui, &label, value, lookup);
        }
        return;
    }

    if items.iter().all(is_scalar_object) {
        for (index, item) in items.iter().enumerate() {
            if let Some(object) = item.as_object() {
                let title = object
                    .get("name")
                    .or_else(|| object.get("title"))
                    .or_else(|| object.get("label"))
                    .and_then(Value::as_str)
                    .map(format_label)
                    .unwrap_or_else(|| format!("Item {}", index + 1));
                object_result_row(ui, &title, object, lookup);
                ui.add_space(10.0);
            }
        }
        return;
    }

    for (index, item) in items.iter().enumerate() {
        let title = item
            .as_object()
            .and_then(|object| {
                object
                    .get("title")
                    .or_else(|| object.get("label"))
                    .or_else(|| object.get("name"))
                    .or_else(|| object.get("phase"))
            })
            .and_then(Value::as_str)
            .map(format_label)
            .unwrap_or_else(|| format!("Item {}", index + 1));
        result_section(ui, &title, |ui| {
            render_value(ui, item, depth + 1, lookup, context)
        });
        ui.add_space(10.0);
    }
}

fn object_result_row(
    ui: &mut egui::Ui,
    title: &str,
    object: &serde_json::Map<String, Value>,
    lookup: &mut Option<LookupTarget>,
) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(title).size(15.0).strong().color(TEXT));
            ui.add_space(8.0);
            for (index, (key, value)) in object.iter().enumerate() {
                if index > 0 {
                    divider(ui);
                    ui.add_space(8.0);
                }
                kv_row_without_divider(ui, key, value, lookup);
                ui.add_space(8.0);
            }
        });
}

fn result_section(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(title).size(14.0).strong().color(TEXT));
            ui.add_space(8.0);
            add_contents(ui);
        });
}

fn kv_row(ui: &mut egui::Ui, key: &str, value: &Value, lookup: &mut Option<LookupTarget>) {
    kv_row_without_divider(ui, key, value, lookup);
    ui.add_space(8.0);
    divider(ui);
    ui.add_space(8.0);
}

fn kv_row_without_divider(
    ui: &mut egui::Ui,
    key: &str,
    value: &Value,
    lookup: &mut Option<LookupTarget>,
) {
    let text = value_text(value);
    let label = format_label(key);
    ui.vertical(|ui| {
        if ui.available_width() < 420.0 {
            ui.label(
                egui::RichText::new(label)
                    .size(13.0)
                    .strong()
                    .color(TEXT_MUTED),
            );
            value_widget(ui, key, value, text, lookup);
        } else {
            ui.horizontal(|ui| {
                let label_width = 160.0_f32.min(ui.available_width() * 0.35);
                ui.add_sized(
                    [label_width, 20.0],
                    egui::Label::new(
                        egui::RichText::new(label)
                            .size(13.0)
                            .strong()
                            .color(TEXT_MUTED),
                    ),
                );
                value_widget(ui, key, value, text, lookup);
            });
        }
    });
}

fn value_widget(
    ui: &mut egui::Ui,
    key: &str,
    value: &Value,
    text: String,
    lookup: &mut Option<LookupTarget>,
) {
    if let Some(target) = lookup_target_for_field(key, &text) {
        lookup_link(ui, &text, target, lookup);
    } else if is_long_token(value) {
        ui.add(
            egui::Label::new(egui::RichText::new(text).color(TEXT).monospace())
                .selectable(true)
                .wrap(),
        );
    } else {
        ui.add(
            egui::Label::new(
                egui::RichText::new(text)
                    .color(value_color(value))
                    .monospace(),
            )
            .wrap(),
        );
    }
}

fn error_panel(ui: &mut egui::Ui, error: &str) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(45, 25, 23))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(170, 76, 58)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Error")
                    .size(14.0)
                    .strong()
                    .color(egui::Color32::from_rgb(244, 143, 116)),
            );
            ui.add_space(8.0);
            ui.add(egui::Label::new(egui::RichText::new(error).color(TEXT)).wrap());
        });
}

fn warning_panel(ui: &mut egui::Ui, title: &str, message: &str) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(47, 36, 20))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(179, 126, 52),
        ))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(title)
                    .size(14.0)
                    .strong()
                    .color(egui::Color32::from_rgb(247, 181, 83)),
            );
            ui.add_space(8.0);
            ui.add(egui::Label::new(egui::RichText::new(message).color(TEXT)).wrap());
        });
}

fn lookup_target_for_field(label: &str, value: &str) -> Option<LookupTarget> {
    let label = lookup_label_key(label);
    let value = clean_lookup_value(value)?;
    if label.contains("blockid") || label == "block" {
        return parse_lookup_block(value).map(LookupTarget::Block);
    }
    if is_transaction_lookup_label(&label) {
        return Some(LookupTarget::Transaction(normalize_hash_lookup_value(
            value,
        )?));
    }
    if is_account_lookup_label(&label) {
        return Some(LookupTarget::Account(normalize_account_lookup_value(
            &label, value,
        )?));
    }
    None
}

fn lookup_label_key(label: &str) -> String {
    label
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn clean_lookup_value(value: &str) -> Option<&str> {
    let value = value.trim().trim_matches(|ch| matches!(ch, '"' | '`'));
    if value.is_empty() || value == "-" || value.contains("...") || value.contains(' ') {
        None
    } else {
        Some(value)
    }
}

fn is_transaction_lookup_label(label: &str) -> bool {
    !label.contains("prehash")
        && (label == "hash"
            || label.contains("transactionhash")
            || label.contains("txhash")
            || label.contains("deploymenttxhash")
            || label.contains("deploytx"))
}

fn is_account_lookup_label(label: &str) -> bool {
    label.contains("account")
        || label.contains("programid")
        || label.contains("programhex")
        || label.contains("programowner")
        || label == "owner"
}

fn normalize_hash_lookup_value(value: &str) -> Option<String> {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(value.to_owned())
    } else {
        None
    }
}

fn normalize_account_lookup_value(label: &str, value: &str) -> Option<String> {
    if label.contains("hex") || looks_like_32_byte_hex(value) {
        return account_id_base58_from_hex(value);
    }
    Some(value.to_owned())
}

fn account_id_base58_from_hex(value: &str) -> Option<String> {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    let bytes = hex::decode(value).ok()?;
    let bytes: [u8; 32] = bytes.try_into().ok()?;
    Some(AccountId::new(bytes).to_string())
}

fn looks_like_32_byte_hex(value: &str) -> bool {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn parse_lookup_block(value: &str) -> Option<u64> {
    value.trim_start_matches('#').parse().ok()
}

fn dashboard_search_target(value: &str) -> Option<LookupTarget> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Some((kind, target)) = split_dashboard_search_hint(value) {
        return dashboard_search_target_with_hint(kind, target);
    }
    if let Some(block_id) = parse_lookup_block(value) {
        return Some(LookupTarget::Block(block_id));
    }
    if let Some(hash) = normalize_hash_lookup_value(value) {
        return Some(LookupTarget::Transaction(hash));
    }
    normalize_account_lookup_value("account", value).map(LookupTarget::Account)
}

fn split_dashboard_search_hint(value: &str) -> Option<(&str, &str)> {
    if let Some((kind, target)) = value.split_once(':') {
        return Some((kind.trim(), target.trim()));
    }
    let (kind, target) = value.split_once(char::is_whitespace)?;
    let kind = kind.trim();
    if dashboard_search_hint_kind(kind).is_some() {
        Some((kind, target.trim()))
    } else {
        None
    }
}

fn dashboard_search_target_with_hint(kind: &str, value: &str) -> Option<LookupTarget> {
    match dashboard_search_hint_kind(kind)? {
        DashboardSearchHint::Block => parse_lookup_block(value).map(LookupTarget::Block),
        DashboardSearchHint::Transaction => {
            normalize_hash_lookup_value(value).map(LookupTarget::Transaction)
        }
        DashboardSearchHint::Account => {
            normalize_account_lookup_value("account", value).map(LookupTarget::Account)
        }
    }
}

fn dashboard_search_target_label(target: &LookupTarget) -> &'static str {
    match target {
        LookupTarget::Account(_) => "Account",
        LookupTarget::Transaction(_) => "Transaction",
        LookupTarget::Block(_) => "Block",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DashboardSearchHint {
    Account,
    Transaction,
    Block,
}

fn dashboard_search_hint_kind(value: &str) -> Option<DashboardSearchHint> {
    let key = lookup_label_key(value);
    if matches!(key.as_str(), "block" | "blockid" | "height" | "slot") {
        Some(DashboardSearchHint::Block)
    } else if matches!(
        key.as_str(),
        "tx" | "transaction" | "transactionid" | "hash"
    ) {
        Some(DashboardSearchHint::Transaction)
    } else if matches!(
        key.as_str(),
        "account" | "address" | "accountid" | "program"
    ) {
        Some(DashboardSearchHint::Account)
    } else {
        None
    }
}

fn lookup_target_tooltip(target: &LookupTarget) -> String {
    match target {
        LookupTarget::Account(account_id) => format!("Open account {}", short_token(account_id)),
        LookupTarget::Transaction(hash) => format!("Open transaction {}", short_token(hash)),
        LookupTarget::Block(block_id) => format!("Open block #{block_id}"),
    }
}

fn is_scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
    )
}

fn is_scalar_object(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.values().all(is_scalar))
}

fn is_long_token(value: &Value) -> bool {
    let Some(text) = value.as_str() else {
        return false;
    };
    text.len() > 32 && !text.chars().any(char::is_whitespace)
}

fn value_text(value: &Value) -> String {
    match value {
        Value::Null => "-".to_owned(),
        Value::Bool(value) => {
            if *value {
                "yes".to_owned()
            } else {
                "no".to_owned()
            }
        }
        Value::Number(value) => value.to_string(),
        Value::String(value) => {
            if value.is_empty() {
                "-".to_owned()
            } else {
                value.clone()
            }
        }
        Value::Array(items) => format!("{} items", items.len()),
        Value::Object(object) => format!("{} fields", object.len()),
    }
}

fn value_summary(value: &Value) -> String {
    match value {
        Value::Array(items) => format!("{} nested items", items.len()),
        Value::Object(object) => format!("{} nested fields", object.len()),
        _ => value_text(value),
    }
}

fn value_color(value: &Value) -> egui::Color32 {
    match value {
        Value::Bool(true) => GREEN,
        Value::Bool(false) => egui::Color32::from_rgb(218, 92, 70),
        Value::Null => TEXT_MUTED,
        _ => TEXT,
    }
}

fn value_str<'a>(object: &'a Value, key: &str) -> Option<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

fn account_definition_type_from_decode(decode: &Value) -> Option<&str> {
    value_str(decode, "account_type")
}

fn value_u64(object: &Value, key: &str) -> Option<u64> {
    object.get(key).and_then(Value::as_u64)
}

fn value_usize(object: &Value, key: &str) -> Option<usize> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn short_token(value: &str) -> String {
    if value.chars().count() <= 22 {
        return value.to_owned();
    }
    let prefix = value.chars().take(12).collect::<String>();
    let suffix = value
        .chars()
        .rev()
        .take(6)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{prefix}...{suffix}")
}

fn compact_endpoint(endpoint: &str) -> String {
    let endpoint = endpoint
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    if endpoint.chars().count() <= 26 {
        endpoint.to_owned()
    } else {
        short_token(endpoint)
    }
}

fn head_gap_text(sequencer_head: &str, indexer_head: &str) -> String {
    match (sequencer_head.parse::<u64>(), indexer_head.parse::<u64>()) {
        (Ok(sequencer), Ok(indexer)) if sequencer >= indexer => {
            format!("{}", sequencer - indexer)
        }
        (Ok(_), Ok(_)) => "ahead".to_owned(),
        _ => "-".to_owned(),
    }
}

fn dashboard_recent_window_seconds(blocks: &[DashboardBlock]) -> Option<u64> {
    if blocks.len() < 2 {
        return None;
    }
    let min = blocks
        .iter()
        .map(|block| timestamp_seconds(block.timestamp))
        .min()?;
    let max = blocks
        .iter()
        .map(|block| timestamp_seconds(block.timestamp))
        .max()?;
    max.checked_sub(min).filter(|seconds| *seconds > 0)
}

fn timestamp_seconds(timestamp: u64) -> u64 {
    if timestamp > 10_000_000_000 {
        timestamp / 1_000
    } else {
        timestamp
    }
}

fn format_tps(value: f64) -> String {
    if value >= 100.0 {
        format!("{value:.0} tx/s")
    } else if value >= 10.0 {
        format!("{value:.1} tx/s")
    } else {
        format!("{value:.2} tx/s")
    }
}

fn timestamp_label(timestamp: u64) -> String {
    let seconds = timestamp_seconds(timestamp);
    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return seconds.to_string();
    };
    if seconds == 0 || seconds > now.as_secs() {
        seconds.to_string()
    } else {
        format!(
            "{} ago",
            duration_label(Duration::from_secs(now.as_secs() - seconds))
        )
    }
}

fn duration_label(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h", seconds / 3_600)
    } else {
        format!("{}d", seconds / 86_400)
    }
}

fn probe_text(probe: &Value) -> String {
    if probe
        .get("ok")
        .and_then(Value::as_bool)
        .is_some_and(|ok| !ok)
    {
        return "error".to_owned();
    }
    let Some(value) = probe.get("value") else {
        return "-".to_owned();
    };
    if let Some(result) = value.get("result") {
        value_text(result)
    } else {
        value_text(value)
    }
}

fn probe_text_field(output: &Value, service: &str, field: &str) -> String {
    output
        .get(service)
        .and_then(|service| service.get(field))
        .map(probe_text)
        .unwrap_or_else(|| "-".to_owned())
}

fn probe_ok(output: &Value, service: &str, field: &str) -> Option<bool> {
    output
        .get(service)
        .and_then(|service| service.get(field))
        .and_then(|probe| probe.get("ok"))
        .and_then(Value::as_bool)
}

fn overview_payload(output: &Value) -> &Value {
    output.get("overview").unwrap_or(output)
}

fn completion_label(task: &str, output: &Value) -> String {
    if task == "fetching programs"
        && let Some(count) = output.as_array().map(Vec::len)
    {
        return format!("{count} program IDs loaded");
    }
    if let Some(count) = output.as_array().map(Vec::len) {
        return format!("{} complete: {count} items", format_label(task));
    }
    format!("{} complete", format_label(task))
}

fn format_label(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut capitalize = true;
    for ch in value.chars() {
        if matches!(ch, '_' | '-') {
            output.push(' ');
            capitalize = true;
        } else if capitalize {
            output.extend(ch.to_uppercase());
            capitalize = false;
        } else {
            output.push(ch);
        }
    }
    output
}

fn nav_item(ui: &mut egui::Ui, active: bool, label: &str) -> egui::Response {
    let fill = if active { ACCENT_DARK } else { SIDE_BG };
    let color = if active { TEXT } else { TEXT_MUTED };
    let stroke = if active {
        egui::Stroke::new(1.0, ACCENT)
    } else {
        egui::Stroke::new(1.0, BORDER)
    };
    let response = ui.add(
        egui::Button::new(egui::RichText::new(label).size(14.0).color(color))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(8))
            .min_size(egui::vec2(ui.available_width(), 44.0)),
    );
    focus_outline(ui, &response);
    response
}

fn compact_nav_item(ui: &mut egui::Ui, active: bool, label: &str) -> egui::Response {
    let fill = if active { ACCENT_DARK } else { PANEL };
    let color = if active { TEXT } else { TEXT_MUTED };
    let stroke = if active {
        egui::Stroke::new(1.0, ACCENT)
    } else {
        egui::Stroke::new(1.0, BORDER)
    };
    let response = ui.add(
        egui::Button::new(egui::RichText::new(label).size(13.0).color(color))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(8))
            .min_size(egui::vec2(92.0, 44.0)),
    );
    focus_outline(ui, &response);
    response
}

fn status_spinner(ui: &mut egui::Ui, label: &str) {
    egui::Frame::new()
        .fill(ACCENT_DARK)
        .stroke(egui::Stroke::new(1.0, ACCENT))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 7))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(egui::RichText::new(label).size(13.0).color(TEXT));
            });
        });
}

fn status_pill(ui: &mut egui::Ui, label: &str) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 7))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).size(13.0).color(TEXT_MUTED));
        });
}

fn status_dot(ui: &mut egui::Ui, idle: bool) {
    let color = if idle { GREEN } else { ACCENT };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

fn tag_pill(ui: &mut egui::Ui, label: &str) {
    egui::Frame::new()
        .fill(ACCENT_DARK)
        .stroke(egui::Stroke::new(1.0, ACCENT))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).size(12.0).color(TEXT));
        });
}

fn status_chip(ui: &mut egui::Ui, label: &str) {
    egui::Frame::new()
        .fill(ACCENT_DARK)
        .stroke(egui::Stroke::new(1.0, ACCENT))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).size(12.0).strong().color(TEXT));
        });
}

fn sidebar_section_label(ui: &mut egui::Ui, label: &str) {
    ui.label(
        egui::RichText::new(label)
            .size(13.0)
            .strong()
            .color(TEXT_MUTED),
    );
}

fn sidebar_kv(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(12.0)
                .strong()
                .color(TEXT_MUTED),
        );
        ui.add_space(2.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(value)
                    .size(13.0)
                    .monospace()
                    .color(TEXT),
            )
            .truncate(),
        )
        .on_hover_text(value);
    });
}

fn divider(ui: &mut egui::Ui) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0, BORDER);
}

fn network_profile_field(ui: &mut egui::Ui, value: &mut String) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let label_response = ui.add_sized(
                    [78.0, 22.0],
                    egui::Label::new(
                        egui::RichText::new("Network")
                            .size(12.0)
                            .strong()
                            .color(TEXT_MUTED),
                    ),
                );
                let response = egui::ComboBox::from_id_salt("network-profile")
                    .selected_text(network_profile_label(value))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for profile in network_profiles() {
                            ui.selectable_value(value, profile.id.to_owned(), profile.label);
                        }
                        ui.selectable_value(value, CUSTOM_NETWORK_PROFILE.to_owned(), "Custom");
                    })
                    .response
                    .labelled_by(label_response.id);
                focus_outline(ui, &response);
            });
        });
}

fn network_profile_label(profile_id: &str) -> &str {
    if profile_id == CUSTOM_NETWORK_PROFILE {
        return "Custom";
    }
    network_profiles()
        .iter()
        .find(|profile| profile.id == profile_id)
        .map(|profile| profile.label)
        .unwrap_or(profile_id)
}

fn endpoint_field(
    ui: &mut egui::Ui,
    id_salt: &'static str,
    label: &str,
    value: &mut String,
) -> bool {
    let mut changed = false;
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let label_response = ui.add_sized(
                    [78.0, 22.0],
                    egui::Label::new(
                        egui::RichText::new(label)
                            .size(12.0)
                            .strong()
                            .color(TEXT_MUTED),
                    ),
                );
                let response = ui
                    .add(
                        egui::TextEdit::singleline(value)
                            .id_salt(id_salt)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace),
                    )
                    .labelled_by(label_response.id);
                if response.changed() {
                    changed = true;
                }
                focus_outline(ui, &response);
            });
        });
    changed
}

fn action_row(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 8.0);
        add_contents(ui);
    });
}

fn input_action_row(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui, f32, bool)) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            let stacked = ui.available_width() < 560.0;
            if stacked {
                ui.vertical(|ui| {
                    let input_width = ui.available_width();
                    add_contents(ui, input_width, true);
                });
            } else {
                ui.horizontal(|ui| {
                    let action_width = 196.0;
                    let input_width = (ui.available_width() - action_width).max(260.0);
                    add_contents(ui, input_width, false);
                });
            }
        });
}

fn inline_input_action_row(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui, f32, bool)) {
    let stacked = ui.available_width() < 560.0;
    if stacked {
        ui.vertical(|ui| {
            let input_width = ui.available_width();
            add_contents(ui, input_width, true);
        });
    } else {
        ui.horizontal(|ui| {
            let action_width = 196.0;
            let input_width = (ui.available_width() - action_width).max(260.0);
            add_contents(ui, input_width, false);
        });
    }
}

fn account_definition_type_field(
    ui: &mut egui::Ui,
    id_salt: &'static str,
    idl_json: &str,
    value: &mut String,
) {
    let account_names = idl_account_names(idl_json);
    ui.vertical(|ui| {
        let label_response = ui.label(
            egui::RichText::new("DefinitionType")
                .size(13.0)
                .color(TEXT_MUTED),
        );
        let selected = account_definition_type_label(value).to_owned();
        let response = egui::ComboBox::from_id_salt(id_salt)
            .selected_text(selected)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                ui.selectable_value(value, String::new(), "Auto-detect");
                for name in account_names {
                    ui.selectable_value(value, name.clone(), name);
                }
            })
            .response
            .labelled_by(label_response.id);
        focus_outline(ui, &response);
    });
}

fn labeled_singleline(
    ui: &mut egui::Ui,
    id_salt: &'static str,
    label: &str,
    value: &mut String,
    hint: &str,
) {
    ui.vertical(|ui| {
        let label_response = ui.label(egui::RichText::new(label).size(13.0).color(TEXT_MUTED));
        let response = ui
            .add(
                egui::TextEdit::singleline(value)
                    .id_salt(id_salt)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            )
            .labelled_by(label_response.id);
        focus_outline(ui, &response);
    });
}

fn labeled_multiline(
    ui: &mut egui::Ui,
    id_salt: &'static str,
    label: &str,
    value: &mut String,
    rows: usize,
) {
    let label_response = ui.label(egui::RichText::new(label).size(13.0).color(TEXT_MUTED));
    egui::Frame::new()
        .fill(INPUT)
        .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            let response = ui
                .add(
                    egui::TextEdit::multiline(value)
                        .id_salt(id_salt)
                        .font(egui::TextStyle::Monospace)
                        .hint_text(multiline_hint(label))
                        .desired_rows(rows)
                        .desired_width(f32::INFINITY),
                )
                .labelled_by(label_response.id);
            focus_outline(ui, &response);
        });
}

fn multiline_hint(label: &str) -> &'static str {
    match label {
        "Params JSON" => "[]",
        "IDL JSON" | "Transaction IDL JSON" | "Account IDL JSON" | "Program IDL JSON" => "{ ... }",
        "Instruction words" => "1, 2, 3",
        "Account IDs" => "account-a, account-b",
        "Data hex" | "Event data hex" => "0x...",
        _ => "",
    }
}

fn primary_button_enabled(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let fill = if enabled { ACCENT } else { PANEL };
    let text_color = if enabled { ACCENT_TEXT } else { TEXT_MUTED };
    let stroke = if enabled {
        egui::Stroke::new(1.0, ACCENT)
    } else {
        egui::Stroke::new(1.0, BORDER)
    };
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(
            egui::RichText::new(label)
                .size(14.0)
                .strong()
                .color(text_color),
        )
        .fill(fill)
        .stroke(stroke)
        .corner_radius(egui::CornerRadius::same(8))
        .min_size(egui::vec2(112.0, 42.0)),
    );
    focus_outline(ui, &response);
    response
}

fn secondary_button_enabled(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let color = if enabled { TEXT } else { TEXT_MUTED };
    let stroke = if enabled {
        egui::Stroke::new(1.0, BORDER_STRONG)
    } else {
        egui::Stroke::new(1.0, BORDER)
    };
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).size(14.0).color(color))
            .fill(PANEL)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(8))
            .min_size(egui::vec2(92.0, 38.0)),
    );
    focus_outline(ui, &response);
    response
}

fn focus_outline(ui: &egui::Ui, response: &egui::Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            9,
            egui::Stroke::new(2.0, ACCENT),
            egui::StrokeKind::Outside,
        );
    }
}

async fn dashboard_report(
    sequencer_endpoint: &str,
    indexer_endpoint: &str,
) -> Result<DashboardReport> {
    let overview_report = overview(sequencer_endpoint, indexer_endpoint).await;
    let mut latest_blocks = Vec::with_capacity(8);
    let mut latest_transactions = Vec::with_capacity(12);
    let mut block_errors = Vec::new();

    match last_sequencer_block_id(sequencer_endpoint).await {
        Ok(head) => {
            let start = head.saturating_sub(7);
            for block_id in (start..=head).rev() {
                match sequencer_block(sequencer_endpoint, block_id).await {
                    Ok(Some(block)) => {
                        for transaction in &block.transactions {
                            if latest_transactions.len() >= 12 {
                                break;
                            }
                            latest_transactions.push(DashboardTransaction {
                                block_id: block.block_id,
                                hash: transaction.hash.clone(),
                                kind: transaction.kind.clone(),
                                program_id_hex: transaction.program_id_hex.clone(),
                                account_count: transaction.account_ids.len(),
                                instruction_words: transaction.instruction_data.len(),
                            });
                        }
                        latest_blocks.push(DashboardBlock {
                            block_id: block.block_id,
                            timestamp: block.timestamp,
                            bedrock_status: block.bedrock_status,
                            tx_count: block.tx_count,
                            decode_warning: block.decode_warning,
                        });
                    }
                    Ok(None) => block_errors.push(format!("sequencer block {block_id} not found")),
                    Err(error) => block_errors.push(format!(
                        "failed to fetch sequencer block {block_id}: {error:#}"
                    )),
                }
            }
        }
        Err(error) => block_errors.push(format!(
            "failed to fetch latest sequencer block id: {error:#}"
        )),
    }
    let recent_transaction_count = latest_blocks.iter().map(|block| block.tx_count).sum();
    let recent_window_seconds = dashboard_recent_window_seconds(&latest_blocks);
    let recent_tps = recent_window_seconds
        .filter(|seconds| *seconds > 0)
        .map(|seconds| recent_transaction_count as f64 / seconds as f64);

    Ok(DashboardReport {
        overview: overview_report,
        recent_transaction_count,
        recent_tps,
        recent_window_seconds,
        latest_blocks,
        latest_transactions,
        block_errors,
    })
}

fn run_async<T: Send + 'static>(
    future: impl std::future::Future<Output = Result<T>> + Send + 'static,
) -> Result<T> {
    tokio::runtime::Runtime::new()
        .context("failed to create tokio runtime")?
        .block_on(future)
}

fn optional_text(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    if value.is_empty() { None } else { Some(value) }
}

fn has_text(value: &str) -> bool {
    !value.trim().is_empty()
}

fn parse_words(value: &str) -> Result<Vec<u32>> {
    let raw = value.trim();
    if raw.starts_with('[') {
        return serde_json::from_str(raw).context("failed to parse instruction words JSON array");
    }

    raw.split([',', ' ', '\n', '\t'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            word.parse::<u32>()
                .with_context(|| format!("invalid instruction word `{word}`"))
        })
        .collect()
}

fn parse_accounts(value: &str) -> Result<Vec<String>> {
    let raw = value.trim();
    if raw.is_empty() {
        return Ok(vec![]);
    }
    if raw.starts_with('[') {
        return serde_json::from_str(raw).context("failed to parse accounts JSON array");
    }
    Ok(raw
        .split([',', ' ', '\n', '\t'])
        .filter(|account| !account.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

#[cfg(test)]
mod gui_tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct MemoryStorage {
        values: BTreeMap<String, String>,
    }

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.values.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.values.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.values.remove(key);
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn custom_resize_axes_map_to_viewport_directions() {
        assert_eq!(
            ResizeAxis::East.viewport_direction(),
            egui::viewport::ResizeDirection::East
        );
        assert_eq!(
            ResizeAxis::South.viewport_direction(),
            egui::viewport::ResizeDirection::South
        );
        assert_eq!(
            ResizeAxis::SouthEast.viewport_direction(),
            egui::viewport::ResizeDirection::SouthEast
        );
    }

    #[test]
    fn endpoint_result_does_not_need_scroll() {
        let value = serde_json::json!({
            "network_profile": "default",
            "sequencer_endpoint": "https://testnet.lez.logos.co/",
            "indexer_endpoint": "http://127.0.0.1:8779/",
        });

        assert_eq!(estimated_result_rows(&value), 3);
        assert!(!result_value_needs_scroll(&value));
    }

    #[test]
    fn navigation_exposes_primary_explorer_workflows() {
        assert_eq!(
            View::ALL.map(|(_, label)| label),
            [
                "Dashboard",
                "Sequencer",
                "Accounts",
                "Programs",
                "Indexer",
                "Settings"
            ]
        );
    }

    #[test]
    fn config_shortcut_opens_config_without_raw_result() {
        let mut app = LogosInspectorApp {
            view: View::Indexer,
            ..Default::default()
        };

        app.open_config();

        assert_eq!(app.view, View::Network);
        assert!(app.output.is_none());
        assert_eq!(app.result_scope, None);
        assert!(!app.has_visible_result());
    }

    #[test]
    fn endpoint_edits_wait_for_apply_before_changing_active_config() {
        let app = LogosInspectorApp {
            draft_network_profile: CUSTOM_NETWORK_PROFILE.to_owned(),
            draft_sequencer_url: "https://sequencer.example.invalid/".to_owned(),
            ..Default::default()
        };

        assert_eq!(app.network_profile, DEFAULT_NETWORK_PROFILE);
        assert_eq!(app.sequencer_url, DEFAULT_SEQUENCER_ENDPOINT);
        assert!(app.has_pending_network_config());
    }

    #[test]
    fn reconnect_state_reset_clears_stale_connection_state() {
        let (_sender, overview_receiver) = mpsc::channel();
        let mut app = LogosInspectorApp {
            pending: Some("loading overview".to_owned()),
            overview_receiver: Some(overview_receiver),
            overview_output: Some(serde_json::json!({"old": true})),
            output: Some(serde_json::json!({"stale": true})),
            program_ids: vec![serde_json::json!("program")],
            program_ids_error: Some("old error".to_owned()),
            output_error: Some("old output error".to_owned()),
            result_label: Some("Old result".to_owned()),
            result_scope: Some(ResultScope::Sequencer(SequencerTab::Blocks)),
            last_overview_refresh: Some(Instant::now()),
            last_overview_success: Some(Instant::now()),
            scroll_result_into_view: true,
            draft_network_profile: "local".to_owned(),
            draft_sequencer_url: "http://127.0.0.1:3040/".to_owned(),
            draft_indexer_url: DEFAULT_INDEXER_ENDPOINT.to_owned(),
            ..Default::default()
        };

        assert!(app.activate_network_config());
        app.clear_connection_state();

        assert_eq!(app.network_profile, "local");
        assert_eq!(app.sequencer_url, "http://127.0.0.1:3040/");
        assert!(app.pending.is_none());
        assert!(app.receiver.is_none());
        assert!(app.overview_receiver.is_none());
        assert!(app.overview_output.is_none());
        assert!(app.output.is_none());
        assert!(app.program_ids.is_empty());
        assert!(app.program_ids_error.is_none());
        assert!(app.output_error.is_none());
        assert!(app.result_label.is_none());
        assert!(app.result_scope.is_none());
        assert!(app.last_overview_refresh.is_none());
        assert!(app.last_overview_success.is_none());
        assert!(!app.scroll_result_into_view);
    }

    #[test]
    fn overview_result_updates_metrics_without_showing_raw_panel() {
        let app = LogosInspectorApp {
            overview_output: Some(serde_json::json!({
                "network_profile": "default",
                "sequencer_endpoint": "https://testnet.lez.logos.co/",
                "indexer_endpoint": "http://127.0.0.1:8779/",
            })),
            ..Default::default()
        };

        assert!(app.overview_output.is_some());
        assert!(!app.has_visible_result());
    }

    #[test]
    fn idl_account_names_reads_loaded_idl_account_options() {
        let names = idl_account_names(
            r#"{
                "accounts": [
                    {"name": "Wallet", "type": {"kind": "struct", "fields": []}},
                    {"name": "Vault", "type": {"kind": "struct", "fields": []}}
                ]
            }"#,
        );

        assert_eq!(names, vec!["Wallet".to_owned(), "Vault".to_owned()]);
        assert!(idl_account_names("{").is_empty());
    }

    #[test]
    fn account_definition_type_uses_decoded_account_type() {
        let decode = serde_json::json!({"account_type": "Vault"});

        assert_eq!(account_definition_type_from_decode(&decode), Some("Vault"));
        assert_eq!(account_definition_type_label(""), "Auto-detect");
        assert_eq!(account_definition_type_label("Wallet"), "Wallet");
    }

    #[test]
    fn account_output_tab_defaults_to_decoded_when_idl_output_exists() {
        let output = serde_json::json!({
            "account": {
                "account_id": "acct",
                "account": {},
                "data_hex": "0102"
            },
            "decode": {
                "account_type": "Vault",
                "rows": [],
                "decoded": {}
            }
        });

        assert_eq!(
            default_account_output_tab(&output),
            Some(AccountOutputTab::Decoded)
        );
    }

    #[test]
    fn account_output_tab_defaults_to_detail_without_idl_output() {
        let output = serde_json::json!({
            "account_id": "acct",
            "account": {},
            "data_hex": "0102"
        });

        assert_eq!(
            default_account_output_tab(&output),
            Some(AccountOutputTab::Detail)
        );
    }

    #[test]
    fn dashboard_search_routes_blocks_transactions_and_accounts() {
        let hash = "ab".repeat(32);
        let account = "11111111111111111111111111111111";
        let account_hex = "00".repeat(32);
        let program_account = AccountId::new([0; 32]).to_string();

        assert_eq!(dashboard_search_target("42"), Some(LookupTarget::Block(42)));
        assert_eq!(
            dashboard_search_target(&format!("tx:{hash}")),
            Some(LookupTarget::Transaction(hash.clone()))
        );
        assert_eq!(
            dashboard_search_target(account),
            Some(LookupTarget::Account(account.to_owned()))
        );
        assert_eq!(
            dashboard_search_target(&format!("account:{account_hex}")),
            Some(LookupTarget::Account(program_account))
        );
    }

    #[test]
    fn dashboard_recent_window_uses_timestamp_seconds() {
        let blocks = vec![
            DashboardBlock {
                block_id: 2,
                timestamp: 20_000,
                bedrock_status: "Finalized".to_owned(),
                tx_count: 3,
                decode_warning: None,
            },
            DashboardBlock {
                block_id: 1,
                timestamp: 10_000,
                bedrock_status: "Finalized".to_owned(),
                tx_count: 2,
                decode_warning: None,
            },
        ];

        assert_eq!(dashboard_recent_window_seconds(&blocks), Some(10_000));
        assert_eq!(format_tps(0.25), "0.25 tx/s");
    }

    #[test]
    fn saved_idl_definitions_are_persisted() {
        let mut app = LogosInspectorApp {
            registered_idls: vec![RegisteredIdl {
                name: "Wallet IDL".to_owned(),
                program_id: Some("wallet-program".to_owned()),
                json: r#"{"accounts":[{"name":"Wallet"}]}"#.to_owned(),
            }],
            active_idl_name: Some("Wallet IDL".to_owned()),
            ..Default::default()
        };
        let mut storage = MemoryStorage::default();

        eframe::App::save(&mut app, &mut storage);
        let saved = eframe::get_value::<PersistedIdlState>(&storage, IDL_STORAGE_KEY);

        assert!(saved.is_some());
        let saved = saved.unwrap_or_default();
        assert_eq!(saved.active_idl_name.as_deref(), Some("Wallet IDL"));
        assert_eq!(saved.registered_idls.len(), 1);
        assert_eq!(
            saved.registered_idls.first().map(|idl| idl.name.as_str()),
            Some("Wallet IDL")
        );
    }

    #[test]
    fn lookup_target_for_field_maps_lookupable_fields() {
        let account = "11111111111111111111111111111111";
        let hash = "ab".repeat(32);
        let program_hex = "00".repeat(32);
        let program_account = AccountId::new([0; 32]).to_string();

        assert_eq!(
            lookup_target_for_field("account[0]", account),
            Some(LookupTarget::Account(account.to_owned()))
        );
        assert_eq!(
            lookup_target_for_field("deployment_tx_hash", &hash),
            Some(LookupTarget::Transaction(hash.clone()))
        );
        assert_eq!(
            lookup_target_for_field("Block ID", "#42"),
            Some(LookupTarget::Block(42))
        );
        assert_eq!(
            lookup_target_for_field("program_id_hex", &program_hex),
            Some(LookupTarget::Account(program_account))
        );
        assert_eq!(lookup_target_for_field("message_prehash", &hash), None);
    }

    #[test]
    fn background_overview_update_does_not_clear_foreground_task() {
        let (sender, receiver) = mpsc::channel();
        let mut app = LogosInspectorApp {
            pending: Some("fetching block".to_owned()),
            receiver: None,
            overview_receiver: Some(receiver),
            output: Some(serde_json::json!({"block": 1})),
            result_scope: Some(ResultScope::Sequencer(SequencerTab::Blocks)),
            ..Default::default()
        };

        let send_result = sender.send(Ok(serde_json::json!({"overview": true})));
        assert!(send_result.is_ok());
        app.receive_overview_task();

        assert_eq!(app.pending.as_deref(), Some("fetching block"));
        assert!(app.overview_receiver.is_none());
        assert_eq!(
            app.overview_output,
            Some(serde_json::json!({"overview": true}))
        );
        assert_eq!(app.output, Some(serde_json::json!({"block": 1})));
        assert_eq!(
            app.result_scope,
            Some(ResultScope::Sequencer(SequencerTab::Blocks))
        );
    }

    #[test]
    fn large_nested_result_needs_scroll() {
        let value = serde_json::json!({
            "trace": (0..12)
                .map(|index| serde_json::json!({
                    "step": index,
                    "program": "example",
                    "status": "ok",
                }))
                .collect::<Vec<_>>(),
        });

        assert!(result_value_needs_scroll(&value));
    }

    #[test]
    fn long_error_needs_scroll() {
        let error = (0..12)
            .map(|index| format!("line {index}: failure detail"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(result_error_needs_scroll(&error));
    }
}
