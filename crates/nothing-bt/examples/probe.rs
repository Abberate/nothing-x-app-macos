//! Low-level probe: lists the SDP services of a paired device, opens the Nothing SPP
//! RFCOMM channel and sends a few *read-only* requests, printing every byte received.
//!
//! cargo run -p nothing-bt --example probe -- [address] [--thread]

use std::ffi::c_void;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send};
use objc2_core_foundation::{CFRunLoop, kCFRunLoopDefaultMode};
use objc2_foundation::NSString;
use objc2_io_bluetooth::{
    IOBluetoothDevice, IOBluetoothRFCOMMChannel, IOBluetoothRFCOMMChannelDelegate,
    IOBluetoothSDPServiceRecord, IOBluetoothSDPUUID,
};

const SPP_UUID: [u8; 16] = uuid("aeac4a03-dff5-498f-843a-34487cf133eb");
const FASTPAIR_UUID: [u8; 16] = uuid("df21fe2c-2515-4fdb-8886-f12c4d67927c");

const fn hex(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("bad hex"),
    }
}

const fn uuid(s: &str) -> [u8; 16] {
    let b = s.as_bytes();
    let mut out = [0u8; 16];
    let (mut i, mut o) = (0, 0);
    while i < b.len() {
        if b[i] == b'-' {
            i += 1;
            continue;
        }
        out[o] = (hex(b[i]) << 4) | hex(b[i + 1]);
        o += 1;
        i += 2;
    }
    out
}

enum Ev {
    Data(Vec<u8>),
    Closed,
    Opened(i32),
}

struct Ivars {
    tx: mpsc::Sender<Ev>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "NXProbeRfcommDelegate"]
    #[ivars = Ivars]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl IOBluetoothRFCOMMChannelDelegate for Delegate {
        #[unsafe(method(rfcommChannelData:data:length:))]
        unsafe fn rfcomm_data(
            &self,
            _ch: Option<&IOBluetoothRFCOMMChannel>,
            data: *mut c_void,
            len: usize,
        ) {
            let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, len) }.to_vec();
            let _ = self.ivars().tx.send(Ev::Data(bytes));
        }

        #[unsafe(method(rfcommChannelOpenComplete:status:))]
        unsafe fn rfcomm_open(&self, _ch: Option<&IOBluetoothRFCOMMChannel>, status: i32) {
            let _ = self.ivars().tx.send(Ev::Opened(status));
        }

        #[unsafe(method(rfcommChannelClosed:))]
        unsafe fn rfcomm_closed(&self, _ch: Option<&IOBluetoothRFCOMMChannel>) {
            let _ = self.ivars().tx.send(Ev::Closed);
        }
    }
);

impl Delegate {
    fn new(tx: mpsc::Sender<Ev>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(Ivars { tx });
        unsafe { msg_send![super(this), init] }
    }
}

fn crc16(buf: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in buf {
        crc ^= b as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

fn frame(cmd: u16, op: u8, payload: &[u8]) -> Vec<u8> {
    let mut f = vec![0x55, 0x60, 0x01];
    f.extend_from_slice(&cmd.to_le_bytes());
    f.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    f.push(op);
    f.extend_from_slice(payload);
    let crc = crc16(&f);
    f.extend_from_slice(&crc.to_le_bytes());
    f
}

fn hexs(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn pump(secs: f64) {
    CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, secs, false);
}

fn channel_for(device: &IOBluetoothDevice, uuid: &[u8; 16]) -> Option<u8> {
    unsafe {
        let u = IOBluetoothSDPUUID::uuidWithBytes_length(uuid.as_ptr() as *const c_void, 16)?;
        let rec = device.getServiceRecordForUUID(Some(&u))?;
        let mut ch: u8 = 0;
        (rec.getRFCOMMChannelID(&mut ch) == 0).then_some(ch)
    }
}

fn run(address: &str) {
    unsafe {
        let device = IOBluetoothDevice::deviceWithAddressString(Some(&NSString::from_str(address)))
            .expect("device");
        // The IOBluetooth shim sits on top of CoreBluetooth: let it reach "powered on".
        let skip_fp = std::env::args().any(|a| a == "--no-fp");
        pump(1.5);
        println!(
            "device: {} connected={} paired={}",
            device.name(),
            device.isConnected(),
            device.isPaired()
        );

        if let Some(services) = device.services() {
            for obj in services.iter() {
                let obj: Retained<AnyObject> = obj;
                if let Ok(rec) = obj.downcast::<IOBluetoothSDPServiceRecord>() {
                    let mut ch: u8 = 0;
                    let has = rec.getRFCOMMChannelID(&mut ch) == 0;
                    println!(
                        "  service {:?} rfcomm={}",
                        rec.getServiceName().map(|n| n.to_string()),
                        if has { ch.to_string() } else { "-".into() }
                    );
                }
            }
        } else {
            println!("  (no cached SDP services)");
        }

        let spp = channel_for(&device, &SPP_UUID);
        let fp = channel_for(&device, &FASTPAIR_UUID);
        println!("SPP channel: {spp:?}  FastPair channel: {fp:?}");

        let (tx, rx) = mpsc::channel();
        let delegate = Delegate::new(tx.clone());

        // --- Fast Pair model id ---
        if let Some(fp) = fp.filter(|_| !skip_fp) {
            let mut chan: Option<Retained<IOBluetoothRFCOMMChannel>> = None;
            let r = device.openRFCOMMChannelSync_withChannelID_delegate(
                Some(&mut chan),
                fp,
                Some(&**delegate as &NSObject as &AnyObject),
            );
            println!("open fastpair ch {fp}: {r:#x}");
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_millis(1500) {
                pump(0.05);
                while let Ok(ev) = rx.try_recv() {
                    match ev {
                        Ev::Data(d) => println!("  FP RX {}", hexs(&d)),
                        Ev::Closed => println!("  FP closed"),
                        Ev::Opened(s) => println!("  FP opened {s:#x}"),
                    }
                }
            }
            if let Some(c) = chan {
                c.closeChannel();
            }
            pump(0.2);
            while rx.try_recv().is_ok() {}
        }

        let Some(spp) = spp else {
            println!("no SPP channel");
            return;
        };
        let mut chan: Option<Retained<IOBluetoothRFCOMMChannel>> = None;
        let r = device.openRFCOMMChannelSync_withChannelID_delegate(
            Some(&mut chan),
            spp,
            Some(&**delegate as &NSObject as &AnyObject),
        );
        println!("open SPP ch {spp}: {r:#x}");
        let chan = chan.expect("channel");
        println!("mtu={}", chan.getMTU());

        // read-only requests
        let reqs: [(u16, &str); 8] = [
            (0xC007, "battery"),
            (0xC042, "firmware"),
            (0xC050, "listening mode"),
            (0xC01E, "anc"),
            (0xC04E, "ultra bass"),
            (0xC00E, "in-ear"),
            (0xC041, "latency"),
            (0xC006, "serial"),
        ];
        for (i, (cmd, name)) in reqs.iter().enumerate() {
            let mut f = frame(*cmd, (i + 1) as u8, &[]);
            println!("TX {name}: {}", hexs(&f));
            let r = chan.writeSync_length(f.as_mut_ptr() as *mut c_void, f.len() as u16);
            if r != 0 {
                println!("  write err {r:#x}");
            }
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_millis(400) {
                pump(0.02);
                while let Ok(ev) = rx.try_recv() {
                    if let Ev::Data(d) = ev {
                        println!("  RX {}", hexs(&d));
                    }
                }
            }
        }
        chan.closeChannel();
        pump(0.3);
        drop(tx);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let address = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned()
        .expect("usage: probe <bluetooth address> [--thread]");
    if args.iter().any(|a| a == "--thread") {
        println!("(running on a background thread)");
        std::thread::spawn(move || run(&address)).join().unwrap();
    } else {
        run(&address);
    }
}
