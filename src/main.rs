use std::time::{SystemTime, Duration, UNIX_EPOCH};
use std::collections::BTreeMap;
use std::sync::RwLock;
use std::thread;

use sysinfo::{NetworkExt, NetworksExt, System, SystemExt, CpuExt, DiskExt};
use actix_web::{get, web, App, HttpServer, HttpResponse, http::header::ContentType};
use serde::Serialize;

const SNAPSHOTS: RwLock<Vec<SystemSnapshot>> = RwLock::new(Vec::new());
const UPDATE_INTERVAL: Duration = Duration::from_secs(5);
const MAX_SNAPSHOTS: usize = 60;

#[derive(Serialize)]
struct SystemSnapshot {
    time: u128,
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

#[get("/data")]
async fn get_data() -> HttpResponse {
    let body = serde_json::to_string(&*SNAPSHOTS.read().unwrap()).unwrap();
    HttpResponse::Ok().content_type(ContentType::json()).body(body)
}

#[actix_web::main]
async fn main() {
    let mut sys = System::new_all();

    loop {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|x| x.as_millis()).unwrap_or(0);
        sys.refresh_all();

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

        println!("\n{:?}", serde_json::to_string(&SystemSnapshot { time: now, memory, swap, cpus, disks, networks }).unwrap());

        // SNAPSHOTS.write().unwrap().push(SystemSnapshot { time: now, memory, swap, cpus, disks, networks });
        thread::sleep(UPDATE_INTERVAL);
    }
}
