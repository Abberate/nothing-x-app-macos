//! `nothingctl` — command-line companion, handy for scripting and diagnostics.
//!
//! ```text
//! nothingctl list
//! nothingctl status                      (connect and print everything)
//! nothingctl anc off|transparency|high|mid|low|adaptive
//! nothingctl eq dirac|pop|rock|classical|electronic|vocals|custom
//! nothingctl custom-eq <bass> <mid> <treble>     (-6..6)
//! nothingctl bass on|off [level]
//! nothingctl in-ear on|off   |  latency on|off
//! nothingctl monitor                     (stay connected and print updates)
//! ```
//! Global option: `--device <address>` (defaults to the first connected Nothing/CMF device).

use std::time::{Duration, Instant};

use nothing_protocol::{AncMode, AncStrength, CustomEq, EqPreset};
use nothing_x::session::{Action, Phase, SessionHandle, Snapshot};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::thread::spawn(move || {
        let code = match run(args) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("error: {e}");
                1
            }
        };
        std::process::exit(code);
    });
    nothing_bt::run_main_loop();
}

fn run(mut args: Vec<String>) -> Result<(), String> {
    let device_arg = take_option(&mut args, "--device");
    let command = args.first().cloned().unwrap_or_else(|| "status".into());
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();

    let paired = nothing_bt::paired_devices();
    if command == "list" {
        for d in &paired {
            println!(
                "{}  {:<28} {}{}",
                d.address,
                d.name,
                if d.connected { "connected" } else { "-" },
                if d.looks_like_nothing() {
                    "  (Nothing/CMF)"
                } else {
                    ""
                }
            );
        }
        return Ok(());
    }

    let device = match &device_arg {
        Some(a) => {
            let a = nothing_bt::normalize_address(a);
            paired
                .into_iter()
                .find(|d| d.address == a)
                .ok_or(format!("{a} is not paired"))?
        }
        None => paired
            .into_iter()
            .filter(|d| d.looks_like_nothing())
            .max_by_key(|d| d.connected)
            .ok_or("no Nothing/CMF device paired with this Mac")?,
    };

    let action = parse_action(&command, &rest)?;
    let session = SessionHandle::spawn(|| {});
    session.connect(device.clone());
    eprintln!("connecting to {} ({})…", device.name, device.address);
    wait_ready(&session, Duration::from_secs(25))?;
    // Let the read responses come in.
    std::thread::sleep(Duration::from_millis(1500));

    if let Some(action) = action {
        session.act(action);
        std::thread::sleep(Duration::from_millis(1200));
    }

    if command == "monitor" {
        let mut last = String::new();
        loop {
            let text = render(&session.snapshot());
            if text != last {
                println!("{text}\n");
                last = text;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    println!("{}", render(&session.snapshot()));
    session.shutdown();
    std::thread::sleep(Duration::from_millis(300));
    Ok(())
}

fn take_option(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.remove(i);
    (i < args.len()).then(|| args.remove(i))
}

fn on_off(v: Option<&String>) -> Result<bool, String> {
    match v.map(String::as_str) {
        Some("on" | "1" | "true") => Ok(true),
        Some("off" | "0" | "false") => Ok(false),
        _ => Err("expected on|off".into()),
    }
}

fn parse_action(command: &str, rest: &[String]) -> Result<Option<Action>, String> {
    Ok(Some(match command {
        "status" | "monitor" => return Ok(None),
        "anc" => Action::SetAnc(match rest.first().map(String::as_str) {
            Some("off") => AncMode::Off,
            Some("transparency") => AncMode::Transparency,
            Some("high" | "on") => AncMode::NoiseCancelling(AncStrength::High),
            Some("mid") => AncMode::NoiseCancelling(AncStrength::Mid),
            Some("low") => AncMode::NoiseCancelling(AncStrength::Low),
            Some("adaptive") => AncMode::NoiseCancelling(AncStrength::Adaptive),
            _ => return Err("anc off|transparency|high|mid|low|adaptive".into()),
        }),
        "eq" => Action::SetEqPreset(match rest.first().map(String::as_str) {
            Some("dirac") => EqPreset::DiracOpteo,
            Some("pop") => EqPreset::Pop,
            Some("rock") => EqPreset::Rock,
            Some("classical") => EqPreset::Classical,
            Some("electronic") => EqPreset::Electronic,
            Some("vocals") => EqPreset::EnhanceVocals,
            Some("custom") => EqPreset::Custom,
            _ => return Err("eq dirac|pop|rock|classical|electronic|vocals|custom".into()),
        }),
        "custom-eq" => {
            let v: Vec<f32> = rest.iter().filter_map(|s| s.parse().ok()).collect();
            if v.len() != 3 {
                return Err("custom-eq <bass> <mid> <treble>".into());
            }
            let c = |x: f32| x.round().clamp(-6.0, 6.0);
            Action::SetCustomEq(CustomEq {
                bass: c(v[0]),
                mid: c(v[1]),
                treble: c(v[2]),
            })
        }
        "bass" => {
            let enabled = on_off(rest.first())?;
            let level = rest.get(1).and_then(|l| l.parse().ok()).unwrap_or(3);
            Action::SetUltraBass { enabled, level }
        }
        "in-ear" => Action::SetInEarDetection(on_off(rest.first())?),
        "latency" => Action::SetLowLatency(on_off(rest.first())?),
        other => return Err(format!("unknown command `{other}`")),
    }))
}

fn wait_ready(session: &SessionHandle, timeout: Duration) -> Result<(), String> {
    let start = Instant::now();
    let mut last = Phase::Idle;
    loop {
        let snap = session.snapshot();
        if snap.phase != last {
            eprintln!("  {}", snap.phase.label());
            last = snap.phase.clone();
        }
        if snap.phase.is_ready() {
            return Ok(());
        }
        if start.elapsed() > timeout {
            let log: Vec<_> = snap.log.iter().rev().take(6).rev().cloned().collect();
            return Err(format!(
                "could not connect: {}\n{}",
                snap.phase.label(),
                log.join("\n")
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn render(s: &Snapshot) -> String {
    let st = &s.state;
    let bat = |b: Option<nothing_protocol::BatteryLevel>| {
        b.map(|b| format!("{}%{}", b.percent, if b.charging { " ⚡" } else { "" }))
            .unwrap_or_else(|| "—".into())
    };
    let mut out = vec![
        format!(
            "{} · firmware {} · {}",
            st.model
                .map(|m| format!("{} [{} / {}]", m.name, m.id_hex(), m.config_id()))
                .unwrap_or("?".into()),
            st.firmware.clone().unwrap_or("?".into()),
            s.phase.label()
        ),
        format!(
            "battery  L {}  R {}  case {}",
            bat(st.left),
            bat(st.right),
            bat(st.case)
        ),
        format!("anc      {:?}", st.anc),
        format!("eq       {:?}  custom {:?}", st.eq_preset, st.custom_eq),
        format!(
            "bass     {:?} level {:?}",
            st.ultra_bass_enabled, st.ultra_bass_level
        ),
        format!(
            "in-ear   {:?}  low-latency {:?}",
            st.in_ear_detection, st.low_latency
        ),
        format!(
            "codec    {:?}  spatial {:?}  dual {:?}",
            st.codec, st.spatial, st.dual_enabled
        ),
    ];
    for d in &st.dual_devices {
        out.push(format!(
            "  dual   {} {}{}",
            d.mac_string(),
            d.name,
            if d.connected { " (connected)" } else { "" }
        ));
    }
    let mut g: Vec<_> = st.gestures.iter().collect();
    g.sort();
    out.push(format!(
        "gestures {}",
        g.iter()
            .map(|((d, k, c), a)| format!("{d}/{k}/{c}={a}"))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    out.join("\n")
}
