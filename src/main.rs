use std::time::{SystemTime, Duration, UNIX_EPOCH};
use std::collections::{BTreeMap, VecDeque};
use std::sync::RwLock;
use std::thread;

use sysinfo::{NetworkExt, NetworksExt, System, SystemExt, CpuExt, DiskExt};
use actix_web::{get, web, App, HttpServer, HttpResponse, http::header::ContentType};
use serde::{Serialize, Deserialize};
use superslice::Ext;

const UPDATE_INTERVAL: Duration = Duration::from_secs(60);
const MAX_SNAPSHOTS: usize = 1440;

static SNAPSHOTS: RwLock<VecDeque<(u64, String)>> = RwLock::new(VecDeque::new());

#[derive(Serialize)]
struct SystemSnapshot {
    time: u64,
    memory: MemorySnapshot,
    swap: MemorySnapshot,
    cpus: Vec<CpuSnapshot>,
    disks: BTreeMap<String, DiskSnapshot>,
    networks: BTreeMap<String, NetworkSnapshot>,
}
#[derive(Serialize)]
struct MemorySnapshot {
    total_space: u64,
    used_space: u64,
}
#[derive(Serialize)]
struct NetworkSnapshot {
    received: u64,
    transmitted: u64,
}
#[derive(Serialize)]
struct CpuSnapshot {
    usage: f32,
}
#[derive(Serialize)]
struct DiskSnapshot {
    mount: String,
    total_space: u64,
    used_space: u64,
}

#[derive(Deserialize)]
struct GetDataParams {
    #[serde(default)]
    since: u64,
}
#[get("/data")]
async fn get_data(info: web::Query<GetDataParams>) -> HttpResponse {
    let body = {
        let data = SNAPSHOTS.read().unwrap();
        let (a, b) = data.as_slices();
        let ai = a.lower_bound_by_key(&(info.0.since + 1), |x| x.0);
        let bi = b.lower_bound_by_key(&(info.0.since + 1), |x| x.0);

        let mut res = String::with_capacity(1024);
        res.push('[');
        for (i, x) in (a[ai..].iter().chain(&b[bi..])).enumerate() {
            if i != 0 { res.push(','); }
            res.push_str(&x.1);
        }
        res.push(']');
        res
    };
    HttpResponse::Ok().content_type(ContentType::json()).body(body)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    thread::spawn(move || {
        let mut sys = System::new_all();

        loop {
            sys.refresh_all();

            let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|x| x.as_millis()).unwrap_or(0) as u64;
            let cpus = sys.cpus().iter().map(|x| CpuSnapshot { usage: x.cpu_usage() }).collect();
            let memory = MemorySnapshot {
                total_space: sys.total_memory(),
                used_space: sys.used_memory(),
            };
            let swap = MemorySnapshot {
                total_space: sys.total_swap(),
                used_space: sys.used_swap(),
            };
            let disks = sys.disks().iter().map(|x| (x.name().to_string_lossy().into_owned(), DiskSnapshot {
                mount: x.mount_point().to_string_lossy().into_owned(),
                total_space: x.total_space(),
                used_space: x.total_space().saturating_sub(x.available_space()),
            })).collect();
            let networks = sys.networks().iter().map(|(k, v)| (k.to_owned(), NetworkSnapshot {
                received: v.received(),
                transmitted: v.transmitted(),
            })).collect();
            let snap = serde_json::to_string(&SystemSnapshot { time: now, memory, swap, cpus, disks, networks }).unwrap();

            {
                let mut data = SNAPSHOTS.write().unwrap();
                if data.back().map(|x| now > x.0).unwrap_or(true) {
                    if data.len() >= MAX_SNAPSHOTS {
                        data.pop_front();
                    }
                    data.push_back((now, snap));
                }
            }

            thread::sleep(UPDATE_INTERVAL);
        }
    });

    HttpServer::new(|| {
        App::new().service(get_data)
    }).bind(("127.0.0.1", 6745))?.run().await
}
