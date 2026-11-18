//! Device session: owns the RFCOMM link, speaks the protocol and keeps a shared
//! [`Snapshot`] up to date for the UI (or the CLI).
//!
//! Behaviour mirrors the reference web app:
//! * on connect, identify the exact model through the Fast Pair channel (cached per address);
//! * open the SPP channel and send the read sequence, one request every 100 ms;
//! * reads are sent immediately, writes (`0xF0xx`) one at a time, each waiting for its
//!   ack (same op id) for up to 2 s;
//! * pushes from the buds (battery, ANC, ear-fit…) update the state at any time;
//! * on link loss, reconnect with back-off while the buds are connected to the Mac.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossbeam_channel::{Receiver, Sender, unbounded};
use nothing_bt::{Link, LinkEvent, PairedDevice, Service};
use nothing_protocol::fastpair::MessageStream;
use nothing_protocol::gesture::GestureRecord;
use nothing_protocol::{
    AncMode, Codec, CustomEq, DeviceState, EqPreset, Event, Frame, Reassembler, Request, Side,
    SpatialAudio, cmd, model,
};

const INIT_STEP: Duration = Duration::from_millis(100);
const SET_ACK_TIMEOUT: Duration = Duration::from_millis(2000);
const OPEN_TIMEOUT: Duration = Duration::from_secs(6);
const FASTPAIR_TIMEOUT: Duration = Duration::from_secs(3);
const BATTERY_POLL: Duration = Duration::from_secs(60);
const LOG_LINES: usize = 400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// No device selected.
    Idle,
    /// Device selected but not connected to the Mac (case closed, out of range…).
    WaitingForDevice,
    Identifying,
    Connecting,
    Initializing,
    Ready,
    /// Buds are rebooting after a codec / dual-connection change.
    Rebooting,
    Retrying {
        attempt: u32,
        reason: String,
    },
}

impl Phase {
    pub fn is_ready(&self) -> bool {
        matches!(self, Phase::Ready | Phase::Initializing)
    }

    pub fn label(&self) -> String {
        match self {
            Phase::Idle => "Not connected".into(),
            Phase::WaitingForDevice => "Waiting for the earbuds to connect to this Mac…".into(),
            Phase::Identifying => "Identifying your device…".into(),
            Phase::Connecting => "Connecting…".into(),
            Phase::Initializing => "Setting up your device…".into(),
            Phase::Ready => "Connected".into(),
            Phase::Rebooting => "Rebooting…".into(),
            Phase::Retrying { attempt, reason } => format!("Retrying ({attempt}) — {reason}"),
        }
    }
}

/// Something worth telling the user once (shown as a toast).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    /// The buds did not acknowledge a setting.
    NoAck,
    /// The control channel is ready.
    Connected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Notice {
    pub id: u64,
    pub kind: NoticeKind,
}

/// What the UI renders.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub phase: Phase,
    pub notice: Option<Notice>,
    pub device: Option<PairedDevice>,
    pub state: DeviceState,
    pub ear_fit_running: bool,
    pub log: VecDeque<String>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            notice: None,
            device: None,
            state: DeviceState::new(None),
            ear_fit_running: false,
            log: VecDeque::new(),
        }
    }
}

/// User intents.
#[derive(Debug, Clone)]
pub enum Action {
    SetAnc(AncMode),
    SetEqPreset(EqPreset),
    SetCustomEq(CustomEq),
    SetUltraBass { enabled: bool, level: u8 },
    SetInEarDetection(bool),
    SetLowLatency(bool),
    Ring { side: Side, on: bool },
    SetGesture(GestureRecord),
    StartEarFitTest,
    ResetEarFitTest,
    SetDualEnabled(bool),
    DualConnect { mac: [u8; 6], connect: bool },
    RefreshDualList,
    SetCodec(Codec),
    SetSpatialAudio(SpatialAudio),
    Refresh,
}

#[derive(Debug)]
pub enum Command {
    Connect(PairedDevice),
    Disconnect,
    Act(Action),
    Shutdown,
}

enum Input {
    Command(Command),
    Link { generation: u64, event: LinkEvent },
}

/// Handle used by the UI / CLI.
#[derive(Clone)]
pub struct SessionHandle {
    tx: Sender<Input>,
    shared: Arc<RwLock<Snapshot>>,
}

impl SessionHandle {
    pub fn spawn(on_change: impl Fn() + Send + Sync + 'static) -> SessionHandle {
        let (tx, rx) = unbounded();
        let shared = Arc::new(RwLock::new(Snapshot::default()));
        let session = Session::new(rx, tx.clone(), shared.clone(), Arc::new(on_change));
        std::thread::Builder::new()
            .name("nothing-session".into())
            .spawn(move || session.run())
            .expect("spawn session thread");
        SessionHandle { tx, shared }
    }

    /// A simulated CMF Buds Pro 2, for trying the UI without hardware (`--demo`).
    pub fn spawn_demo(on_change: impl Fn() + Send + Sync + 'static) -> SessionHandle {
        let (tx, rx) = unbounded();
        let shared = Arc::new(RwLock::new(Snapshot::default()));
        let mut session = Session::new(rx, tx.clone(), shared.clone(), Arc::new(on_change));
        session.demo = true;
        std::thread::Builder::new()
            .name("nothing-session-demo".into())
            .spawn(move || session.run())
            .expect("spawn session thread");
        let handle = SessionHandle { tx, shared };
        handle.connect(PairedDevice {
            name: "CMF Buds Pro 2".into(),
            address: "00-00-00-00-00-00".into(),
            connected: true,
            class_of_device: 0,
        });
        handle
    }

    pub fn snapshot(&self) -> Snapshot {
        self.shared.read().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn connect(&self, device: PairedDevice) {
        let _ = self.tx.send(Input::Command(Command::Connect(device)));
    }

    pub fn disconnect(&self) {
        let _ = self.tx.send(Input::Command(Command::Disconnect));
    }

    pub fn act(&self, action: Action) {
        let _ = self.tx.send(Input::Command(Command::Act(action)));
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(Input::Command(Command::Shutdown));
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum LinkKind {
    FastPair,
    Spp,
}

struct Inflight {
    request: Request,
    op: u8,
    sent_at: Instant,
}

struct Session {
    rx: Receiver<Input>,
    tx: Sender<Input>,
    shared: Arc<RwLock<Snapshot>>,
    on_change: Arc<dyn Fn() + Send + Sync>,

    target: Option<PairedDevice>,
    link: Option<Link>,
    link_kind: LinkKind,
    generation: u64,
    open_deadline: Option<Instant>,
    retry_at: Option<Instant>,
    attempt: u32,
    awaiting_reboot: bool,

    reassembler: Reassembler,
    fastpair: MessageStream,
    model_cache: HashMap<String, u32>,

    op: u8,
    init_queue: VecDeque<Request>,
    next_init_at: Option<Instant>,
    set_queue: VecDeque<Request>,
    inflight: Option<Inflight>,
    firmware_retries: u32,
    firmware_check_at: Option<Instant>,
    next_battery_poll: Option<Instant>,
    dual_pages: u32,
    demo: bool,
    demo_fit_at: Option<Instant>,
    started_at: Instant,
    fastpair_attempts: u32,
    notice_id: u64,
}

impl Session {
    fn new(
        rx: Receiver<Input>,
        tx: Sender<Input>,
        shared: Arc<RwLock<Snapshot>>,
        on_change: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            rx,
            tx,
            shared,
            on_change,
            target: None,
            link: None,
            link_kind: LinkKind::Spp,
            generation: 0,
            open_deadline: None,
            retry_at: None,
            attempt: 0,
            awaiting_reboot: false,
            reassembler: Reassembler::new(),
            fastpair: MessageStream::default(),
            model_cache: load_model_cache(),
            op: 0,
            init_queue: VecDeque::new(),
            next_init_at: None,
            set_queue: VecDeque::new(),
            inflight: None,
            firmware_retries: 0,
            firmware_check_at: None,
            next_battery_poll: None,
            dual_pages: 0,
            demo: false,
            demo_fit_at: None,
            started_at: Instant::now(),
            fastpair_attempts: 0,
            notice_id: 0,
        }
    }

    // ---- plumbing ----------------------------------------------------------------

    fn update(&self, f: impl FnOnce(&mut Snapshot)) {
        if let Ok(mut s) = self.shared.write() {
            f(&mut s);
        }
        (self.on_change)();
    }

    fn log(&self, line: impl Into<String>) {
        let line = line.into();
        log::info!("{line}");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() % 86_400)
            .unwrap_or(0);
        let entry = format!(
            "{:02}:{:02}:{:02} {line}",
            stamp / 3600,
            (stamp / 60) % 60,
            stamp % 60
        );
        if let Ok(mut s) = self.shared.write() {
            s.log.push_back(entry);
            while s.log.len() > LOG_LINES {
                s.log.pop_front();
            }
        }
    }

    fn set_phase(&self, phase: Phase) {
        self.update(|s| s.phase = phase);
    }

    fn notify(&mut self, kind: NoticeKind) {
        self.notice_id += 1;
        let notice = Notice {
            id: self.notice_id,
            kind,
        };
        self.update(|s| s.notice = Some(notice));
    }

    fn state(&self) -> DeviceState {
        self.shared
            .read()
            .map(|s| s.state.clone())
            .unwrap_or_default()
    }

    fn next_deadline(&self) -> Option<Instant> {
        let mut deadlines = vec![
            self.open_deadline,
            self.retry_at,
            self.next_init_at,
            self.firmware_check_at,
            self.next_battery_poll,
            self.demo_fit_at,
        ];
        deadlines.push(self.inflight.as_ref().map(|i| i.sent_at + SET_ACK_TIMEOUT));
        deadlines.into_iter().flatten().min()
    }

    fn run(mut self) {
        loop {
            let timeout = self
                .next_deadline()
                .map(|d| d.saturating_duration_since(Instant::now()))
                .unwrap_or(Duration::from_secs(2))
                .min(Duration::from_secs(2));
            match self.rx.recv_timeout(timeout) {
                Ok(Input::Command(Command::Shutdown)) => {
                    self.close_link();
                    return;
                }
                Ok(Input::Command(c)) => self.on_command(c),
                Ok(Input::Link { generation, event }) if generation == self.generation => {
                    self.on_link_event(event)
                }
                Ok(Input::Link { .. }) => {} // stale link
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
            }
            self.on_tick();
        }
    }

    // ---- link management ---------------------------------------------------------

    fn open(&mut self, kind: LinkKind) {
        let Some(target) = self.target.clone() else {
            return;
        };
        self.close_link();
        self.generation += 1;
        let generation = self.generation;
        let tx = self.tx.clone();
        self.link_kind = kind;
        self.reassembler.clear();
        self.fastpair = MessageStream::default();
        let service = match kind {
            LinkKind::FastPair => Service::FastPair,
            LinkKind::Spp => Service::Spp,
        };
        self.open_deadline = Some(
            Instant::now()
                + if kind == LinkKind::FastPair {
                    FASTPAIR_TIMEOUT
                } else {
                    OPEN_TIMEOUT
                },
        );
        self.set_phase(if kind == LinkKind::FastPair {
            Phase::Identifying
        } else {
            Phase::Connecting
        });
        self.log(format!(
            "opening {kind:?} channel on {} ({})",
            target.name, target.address
        ));
        self.link = Some(Link::open(
            &target.address,
            service,
            Box::new(move |event| {
                let _ = tx.send(Input::Link { generation, event });
            }),
        ));
    }

    fn close_link(&mut self) {
        self.generation += 1; // ignore whatever the old link still reports
        if let Some(link) = self.link.take() {
            link.close();
        }
        self.open_deadline = None;
        self.init_queue.clear();
        self.next_init_at = None;
        self.set_queue.clear();
        self.inflight = None;
        self.firmware_check_at = None;
        self.next_battery_poll = None;
    }

    /// Starts (or restarts) the connection procedure for the current target.
    fn begin(&mut self) {
        let Some(target) = self.target.clone() else {
            return;
        };
        self.retry_at = None;
        if self.demo {
            self.demo_connect();
            return;
        }
        // IOBluetooth sits on CoreBluetooth, which rejects channel opens until it has
        // reported "powered on" (about a second after the first Bluetooth call).
        let warm_up = Duration::from_millis(2000);
        if self.started_at.elapsed() < warm_up {
            self.set_phase(Phase::Connecting);
            self.retry_at = Some(self.started_at + warm_up);
            return;
        }
        if !nothing_bt::is_connected(&target.address) {
            self.close_link();
            self.set_phase(Phase::WaitingForDevice);
            self.retry_at = Some(Instant::now() + Duration::from_secs(2));
            return;
        }
        let cached = self
            .model_cache
            .get(&target.address)
            .copied()
            .and_then(model::lookup);
        match cached {
            Some(m) => {
                self.set_model(Some(m));
                self.open(LinkKind::Spp);
            }
            None => {
                self.fastpair_attempts = 1;
                self.open(LinkKind::FastPair);
            }
        }
    }

    /// Fast Pair failed or timed out: try once more, then fall back to the name.
    fn fastpair_failed(&mut self) {
        if self.fastpair_attempts < 2 {
            self.fastpair_attempts += 1;
            self.log("retrying Fast Pair identification");
            self.open(LinkKind::FastPair);
        } else {
            self.fastpair_done(None);
        }
    }

    fn schedule_retry(&mut self, reason: String) {
        self.close_link();
        if self.target.is_none() {
            return;
        }
        self.attempt += 1;
        let delay = if self.awaiting_reboot {
            Duration::from_secs(if self.attempt == 1 { 10 } else { 5 })
        } else {
            Duration::from_secs((1u64 << self.attempt.min(4)).min(10))
        };
        self.log(format!(
            "retry #{} in {}s: {reason}",
            self.attempt,
            delay.as_secs()
        ));
        self.retry_at = Some(Instant::now() + delay);
        let phase = if self.awaiting_reboot {
            Phase::Rebooting
        } else {
            Phase::Retrying {
                attempt: self.attempt,
                reason,
            }
        };
        self.set_phase(phase);
    }

    fn set_model(&self, m: Option<&'static model::ModelInfo>) {
        self.update(|s| {
            let keep = std::mem::take(&mut s.state);
            s.state = DeviceState::new(m.or(keep.model));
        });
    }

    // ---- inputs ------------------------------------------------------------------

    fn on_command(&mut self, c: Command) {
        match c {
            Command::Connect(device) => {
                self.log(format!(
                    "connect requested: {} {}",
                    device.name, device.address
                ));
                self.close_link();
                self.attempt = 0;
                self.awaiting_reboot = false;
                let model = self
                    .model_cache
                    .get(&device.address)
                    .copied()
                    .and_then(model::lookup);
                self.update(|s| {
                    s.device = Some(device.clone());
                    s.state =
                        DeviceState::new(model.or_else(|| model::guess_from_name(&device.name)));
                    s.ear_fit_running = false;
                });
                self.target = Some(device);
                self.begin();
            }
            Command::Disconnect => {
                self.log("disconnect requested");
                self.close_link();
                self.target = None;
                self.retry_at = None;
                self.update(|s| {
                    s.phase = Phase::Idle;
                    s.device = None;
                    s.state = DeviceState::new(None);
                });
            }
            Command::Act(a) => self.on_action(a),
            Command::Shutdown => {}
        }
    }

    fn on_link_event(&mut self, event: LinkEvent) {
        match event {
            LinkEvent::Opened { channel } => {
                self.open_deadline = None;
                self.log(format!("{:?} channel {channel} open", self.link_kind));
                if self.link_kind == LinkKind::Spp {
                    self.attempt = 0;
                    self.awaiting_reboot = false;
                    self.set_phase(Phase::Initializing);
                    self.start_init();
                } else {
                    // The model id is pushed right away; keep the deadline as a timeout.
                    self.open_deadline = Some(Instant::now() + FASTPAIR_TIMEOUT);
                }
            }
            LinkEvent::Data(bytes) => {
                log::trace!("RX raw {:?} {}", self.link_kind, hex(&bytes));
                match self.link_kind {
                    LinkKind::FastPair => self.on_fastpair_data(&bytes),
                    LinkKind::Spp => {
                        for frame in self.reassembler.push(&bytes) {
                            self.on_frame(frame);
                        }
                    }
                }
            }
            LinkEvent::Closed => {
                if self.link_kind == LinkKind::Spp {
                    self.log("SPP channel closed");
                    self.schedule_retry("connection closed".into());
                }
            }
            LinkEvent::Failed(msg) => {
                self.log(format!("{:?} link error: {msg}", self.link_kind));
                if self.link_kind == LinkKind::FastPair {
                    self.fastpair_failed();
                } else {
                    let hint = if msg.contains("open failed") {
                        "the channel may be in use by another app (e.g. Ear (web) in a browser tab)"
                            .to_string()
                    } else {
                        msg
                    };
                    self.schedule_retry(hint);
                }
            }
        }
    }

    fn on_fastpair_data(&mut self, bytes: &[u8]) {
        for msg in self.fastpair.push(bytes) {
            if let Some(id) = msg.model_id() {
                self.log(format!("Fast Pair model id {id:06X}"));
                self.fastpair_done(Some(id));
                return;
            }
        }
    }

    fn fastpair_done(&mut self, id: Option<u32>) {
        self.close_link();
        let Some(target) = self.target.clone() else {
            return;
        };
        let m = id
            .and_then(model::lookup)
            .or_else(|| model::guess_from_name(&target.name));
        if let Some(id) = id {
            self.model_cache.insert(target.address.clone(), id);
            save_model_cache(&self.model_cache);
        }
        match m {
            Some(m) => self.log(format!(
                "model: {} ({}, config {})",
                m.name,
                m.base,
                m.config_id()
            )),
            None => self.log("model unknown; continuing with generic settings"),
        }
        self.set_model(m);
        // Give the stack a moment to tear the Fast Pair channel down.
        std::thread::sleep(Duration::from_millis(300));
        self.open(LinkKind::Spp);
    }

    fn on_tick(&mut self) {
        let now = Instant::now();
        if self.demo_fit_at.is_some_and(|t| now >= t) {
            self.demo_fit_at = None;
            let ev = Event::EarFit {
                left: nothing_protocol::EarFit::Good,
                right: nothing_protocol::EarFit::Good,
            };
            self.update(|s| {
                s.state.apply(&ev);
                s.ear_fit_running = false;
            });
        }
        if self.retry_at.is_some_and(|t| now >= t) {
            self.retry_at = None;
            self.begin();
        }
        if self.open_deadline.is_some_and(|t| now >= t) {
            self.open_deadline = None;
            match self.link_kind {
                LinkKind::FastPair => {
                    self.log("Fast Pair identification timed out");
                    self.fastpair_failed();
                }
                // A busy channel (another app holds it) never reports a failure: the open
                // just never completes.
                LinkKind::Spp => {
                    self.schedule_retry("no answer: channel probably in use by another app".into())
                }
            }
        }
        if self.next_init_at.is_some_and(|t| now >= t) {
            if let Some(req) = self.init_queue.pop_front() {
                self.send(req);
                self.next_init_at = Some(now + INIT_STEP);
            } else {
                self.next_init_at = None;
                self.set_phase(Phase::Ready);
                self.notify(NoticeKind::Connected);
                self.firmware_check_at = Some(now + Duration::from_secs(3));
                self.next_battery_poll = Some(now + BATTERY_POLL);
            }
        }
        if self.firmware_check_at.is_some_and(|t| now >= t) {
            self.firmware_check_at = None;
            if self.state().firmware.is_none() && self.firmware_retries < 3 {
                self.firmware_retries += 1;
                self.send(Request::get(cmd::GET_FIRMWARE));
                self.firmware_check_at = Some(now + Duration::from_secs(3));
            }
        }
        if self.next_battery_poll.is_some_and(|t| now >= t) {
            self.send(Request::get(cmd::GET_BATTERY));
            self.next_battery_poll = Some(now + BATTERY_POLL);
        }
        if let Some(inflight) = &self.inflight
            && now >= inflight.sent_at + SET_ACK_TIMEOUT
        {
            self.log(format!(
                "no ack for {:#06x} (op {}), moving on",
                inflight.request.command, inflight.op
            ));
            self.inflight = None;
            self.notify(NoticeKind::NoAck);
            self.pump_sets();
        }
    }

    // ---- protocol ----------------------------------------------------------------

    fn start_init(&mut self) {
        let state = self.state();
        let caps = state.caps;
        let mut q = VecDeque::new();
        q.push_back(Request::get(cmd::GET_BATTERY));
        if caps.is_some_and(|c| c.listening_mode) {
            q.push_back(Request::get(cmd::GET_LISTENING_MODE));
        } else {
            q.push_back(Request::get(cmd::GET_EQ));
        }
        q.push_back(Request::get(cmd::GET_FIRMWARE));
        q.push_back(Request::get(cmd::GET_SERIAL));
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        q.push_back(Request::time(now as u32));
        if caps.is_none_or(|c| c.in_ear_detection) {
            q.push_back(Request::get(cmd::GET_IN_EAR));
        }
        q.push_back(Request::get(cmd::GET_LATENCY));
        q.push_back(Request::get(cmd::GET_GESTURES));
        q.push_back(Request::get(cmd::GET_ANC));
        q.push_back(Request::get(cmd::GET_ADVANCED_EQ_STATUS));
        if caps.is_some_and(|c| c.bass.is_some()) {
            q.push_back(Request::get(cmd::GET_ULTRA_BASS));
        }
        if let Some(c) = caps {
            if c.custom_eq.is_some() {
                q.push_back(Request::get(cmd::GET_CUSTOM_EQ));
            }
            if c.dual_connection {
                q.push_back(Request::get(cmd::GET_DUAL_ENABLE));
                q.push_back(Request::dual_list(0));
            }
            if c.codecs != 0 {
                q.push_back(Request::get(cmd::GET_CODEC));
            }
            if c.spatial_audio != 0 {
                q.push_back(Request::get(cmd::GET_SPATIAL_AUDIO));
            }
        }
        self.dual_pages = 1;
        self.firmware_retries = 0;
        self.init_queue = q;
        self.next_init_at = Some(Instant::now());
    }

    fn next_op(&mut self) -> u8 {
        self.op = if self.op == 255 { 1 } else { self.op + 1 };
        self.op
    }

    fn send(&mut self, req: Request) {
        if self.demo {
            if req.command == cmd::SET_EAR_FIT_TEST {
                self.demo_fit_at = Some(Instant::now() + Duration::from_millis(2500));
            }
            return;
        }
        if self.link.is_none() || self.link_kind != LinkKind::Spp {
            return;
        }
        if req.is_set() {
            self.set_queue.push_back(req);
            self.pump_sets();
        } else {
            let op = self.next_op();
            self.write(&req, op);
        }
    }

    fn pump_sets(&mut self) {
        if self.inflight.is_some() {
            return;
        }
        let Some(req) = self.set_queue.pop_front() else {
            return;
        };
        let op = self.next_op();
        self.write(&req, op);
        self.inflight = Some(Inflight {
            request: req,
            op,
            sent_at: Instant::now(),
        });
    }

    fn write(&self, req: &Request, op: u8) {
        let bytes = req.encode(op);
        log::debug!("TX {}", hex(&bytes));
        if let Some(link) = &self.link {
            link.write(bytes);
        }
    }

    fn on_frame(&mut self, frame: Frame) {
        log::debug!(
            "RX cmd {:#06x} op {} {}",
            frame.command,
            frame.operation,
            hex(&frame.payload)
        );
        if let Some(inflight) = &self.inflight
            && cmd::is_response_to(frame.command, inflight.request.command)
            && frame.operation == inflight.op
        {
            self.inflight = None;
            self.pump_sets();
        }
        let event = Event::parse(&frame);
        match &event {
            Event::Unknown { command, payload } if !cmd::is_write_ack(*command) => {
                log::debug!("unhandled {command:#06x}: {}", hex(payload));
            }
            Event::Eq(6) => {
                if self.state().custom_eq.is_none() {
                    self.send(Request::get(cmd::GET_CUSTOM_EQ));
                }
            }
            Event::DualList { devices, .. } => {
                let mut macs: Vec<[u8; 6]> =
                    self.state().dual_devices.iter().map(|d| d.mac).collect();
                let before = macs.len();
                for d in devices {
                    if !macs.contains(&d.mac) {
                        macs.push(d.mac);
                    }
                }
                if macs.len() > before && self.dual_pages < 20 {
                    self.dual_pages += 1;
                    self.send(Request::dual_list(macs.len().min(255) as u8));
                }
            }
            Event::DualDevice { mac, .. } => {
                if !self.state().dual_devices.iter().any(|d| d.mac == *mac) {
                    self.refresh_dual_list();
                }
            }
            Event::Firmware(fw) => self.log(format!("firmware {fw}")),
            _ => {}
        }
        let ear_fit = matches!(event, Event::EarFit { .. });
        self.update(|s| {
            s.state.apply(&event);
            if ear_fit {
                s.ear_fit_running = false;
            }
        });
    }

    fn refresh_dual_list(&mut self) {
        self.dual_pages = 1;
        self.update(|s| s.state.dual_devices.clear());
        self.send(Request::dual_list(0));
    }

    fn on_action(&mut self, action: Action) {
        let state = self.state();
        let caps = state.caps;
        match action {
            Action::SetAnc(mode) => {
                self.update(|s| {
                    s.state.anc = Some(mode);
                    if let AncMode::NoiseCancelling(st) = mode {
                        s.state.anc_strength = Some(st);
                    }
                });
                self.send(Request::anc(mode));
            }
            Action::SetEqPreset(preset) => {
                self.update(|s| s.state.eq_preset = Some(preset));
                if caps.is_none_or(|c| c.listening_mode) {
                    self.send(Request::listening_mode(preset));
                } else {
                    self.send(Request::eq(preset as u8));
                }
                if preset == EqPreset::Custom {
                    self.send(Request::get(cmd::GET_CUSTOM_EQ));
                }
            }
            Action::SetCustomEq(eq) => {
                let Some(filters) = caps.and_then(|c| c.custom_eq) else {
                    return;
                };
                self.update(|s| s.state.custom_eq = Some(eq));
                self.send(Request::custom_eq(eq, &filters));
            }
            Action::SetUltraBass { enabled, level } => {
                let Some(scale) = caps.and_then(|c| c.bass) else {
                    return;
                };
                self.update(|s| {
                    s.state.ultra_bass_enabled = Some(enabled);
                    s.state.ultra_bass_level = Some(level);
                });
                self.send(Request::ultra_bass(enabled, level, scale));
            }
            Action::SetInEarDetection(on) => {
                self.update(|s| s.state.in_ear_detection = Some(on));
                self.send(Request::in_ear_detection(on));
            }
            Action::SetLowLatency(on) => {
                self.update(|s| s.state.low_latency = Some(on));
                self.send(Request::low_latency(on));
            }
            Action::Ring { side, on } => {
                self.update(|s| match side {
                    Side::Left => s.state.ringing_left = on,
                    Side::Right => s.state.ringing_right = on,
                });
                self.send(Request::ring(side, on));
            }
            Action::SetGesture(r) => {
                self.update(|s| s.state.record_gesture(r));
                self.send(Request::gesture(r.device, r.common, r.kind, r.action));
            }
            Action::StartEarFitTest => {
                self.update(|s| {
                    s.state.ear_fit = None;
                    s.ear_fit_running = true;
                });
                self.send(Request::ear_fit_test());
            }
            Action::ResetEarFitTest => {
                self.update(|s| {
                    s.state.ear_fit = None;
                    s.ear_fit_running = false;
                });
            }
            Action::SetDualEnabled(on) => {
                let reboot = caps.is_some_and(|c| c.dual_connection_reboot);
                self.update(|s| s.state.dual_enabled = Some(on));
                self.send(Request::dual_enable(on));
                if reboot {
                    self.expect_reboot();
                } else if on {
                    self.refresh_dual_list();
                }
            }
            Action::DualConnect { mac, connect } => {
                self.send(Request::dual_connect(mac, connect));
                self.refresh_dual_list();
            }
            Action::RefreshDualList => self.refresh_dual_list(),
            Action::SetCodec(codec) => {
                self.update(|s| s.state.codec = Some(codec));
                self.send(Request::codec(codec));
                self.expect_reboot();
            }
            Action::SetSpatialAudio(mode) => {
                self.update(|s| s.state.spatial = Some(mode));
                self.send(Request::spatial_audio(mode));
            }
            Action::Refresh => {
                if self.link.is_some() && self.link_kind == LinkKind::Spp {
                    self.start_init();
                } else {
                    self.begin();
                }
            }
        }
    }

    fn demo_connect(&mut self) {
        use nothing_protocol::gesture::{GestureRecord as G, action as a, kind as k};
        use nothing_protocol::message::{AncKind, AncReport};
        use nothing_protocol::{AncStrength, BatteryLevel, DualDevice};
        let model = model::lookup(0x2F45F5);
        let lvl = |p| {
            Some(BatteryLevel {
                percent: p,
                charging: false,
            })
        };
        let g = |device, common, kind, action| G {
            device,
            common,
            kind,
            action,
        };
        let events = vec![
            Event::Battery {
                left: lvl(95),
                right: lvl(95),
                // Scene "case-away": earbuds out of the case, which then can't report.
                case: if std::env::var("NOTHINGX_SCENE").as_deref() == Ok("case-away") {
                    None
                } else {
                    lvl(40)
                },
                single: None,
            },
            Event::Anc(AncReport {
                mode: Some(AncKind::On),
                strength: Some(AncStrength::High),
            }),
            Event::Eq(EqPreset::DiracOpteo as u8),
            Event::CustomEq(CustomEq::default()),
            Event::UltraBass {
                enabled: true,
                wire_level: 6,
            },
            Event::InEarDetection(true),
            Event::LowLatency(true),
            Event::Firmware("1.0.1.74".into()),
            Event::Serial("DEMO0000000000000".into()),
            Event::Codec(Codec::Aac),
            Event::SpatialAudio(SpatialAudio::Off),
            Event::DualEnabled(true),
            Event::DualList {
                total: Some(2),
                devices: vec![
                    DualDevice {
                        mac: [1, 2, 3, 4, 5, 6],
                        name: "MacBook".into(),
                        is_self: true,
                        connected: true,
                    },
                    DualDevice {
                        mac: [6, 5, 4, 3, 2, 1],
                        name: "iPhone".into(),
                        is_self: false,
                        connected: false,
                    },
                ],
            },
            Event::Gestures(vec![
                g(2, 1, k::DOUBLE, a::SKIP_FORWARD),
                g(3, 1, k::DOUBLE, a::SKIP_FORWARD),
                g(2, 1, k::TRIPLE, a::SKIP_BACK),
                g(3, 1, k::TRIPLE, a::SKIP_BACK),
                g(2, 1, k::HOLD, a::NOISE_CONTROL_TRANSPARENCY_NC),
                g(3, 1, k::HOLD, a::NOISE_CONTROL_TRANSPARENCY_NC),
                g(2, 1, k::DOUBLE_HOLD, a::NONE),
                g(3, 1, k::DOUBLE_HOLD, a::NONE),
                g(4, 1, k::SINGLE, a::PLAY_PAUSE),
                g(4, 1, k::DOUBLE, a::SKIP_FORWARD),
                g(4, 9, k::DOUBLE, a::ANSWER_CALL),
                g(4, 1, k::TRIPLE, a::SKIP_BACK),
                g(4, 9, k::TRIPLE, a::HANG_UP_DECLINE),
                g(4, 1, k::HOLD, a::NOISE_CONTROL_ALL),
                g(4, 1, k::ROTATE, a::VOLUME_CONTROL),
            ]),
        ];
        self.update(|s| {
            s.state = DeviceState::new(model);
            for e in &events {
                s.state.apply(e);
            }
            s.phase = Phase::Ready;
        });
        self.log("demo device connected");
    }

    fn expect_reboot(&mut self) {
        self.log("earbuds will reboot; reconnecting afterwards");
        self.awaiting_reboot = true;
        self.attempt = 0;
        self.set_phase(Phase::Rebooting);
    }
}

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

// ---- model cache (address → Fast Pair id) -------------------------------------------

fn cache_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/NothingX/models.txt"))
}

fn load_model_cache() -> HashMap<String, u32> {
    let Some(path) = cache_path() else {
        return HashMap::new();
    };
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (addr, id) = l.split_once(' ')?;
            Some((addr.to_string(), u32::from_str_radix(id.trim(), 16).ok()?))
        })
        .collect()
}

fn save_model_cache(cache: &HashMap<String, u32>) {
    let Some(path) = cache_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let body: String = cache
        .iter()
        .map(|(a, id)| format!("{a} {id:06X}\n"))
        .collect();
    let _ = std::fs::write(path, body);
}
