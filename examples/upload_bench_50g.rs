use reqwest::blocking::Client;
use std::error::Error;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GIB: u64 = 1024 * 1024 * 1024;

struct NodeGuard {
    child: Child,
}

impl Drop for NodeGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn now_run_id() -> Result<u64, Box<dyn Error>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn wait_for_health(client: &Client, api_port: u16, retries: usize) -> Result<(), Box<dyn Error>> {
    let url = format!("http://127.0.0.1:{}/health", api_port);
    for _ in 0..retries {
        if let Ok(resp) = client.get(&url).send() {
            if resp.status().is_success() {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
    Err("ERROR: API did not become healthy".into())
}

fn tail_file(path: &PathBuf, max_bytes: usize) -> String {
    let mut buf = Vec::new();
    if let Ok(mut f) = File::open(path) {
        let _ = f.read_to_end(&mut buf);
    }
    if buf.len() <= max_bytes {
        return String::from_utf8_lossy(&buf).to_string();
    }
    let start = buf.len() - max_bytes;
    String::from_utf8_lossy(&buf[start..]).to_string()
}

fn main() -> Result<(), Box<dyn Error>> {
    let port_api: u16 = 18480;
    let port_p2p: u16 = 18470;
    let port_disc: u16 = 18490;
    let run_id = now_run_id()?;

    let data_dir = PathBuf::from(format!("/mnt/riffcastle/upload-test/50g-file-{}", run_id));
    let input_file = PathBuf::from(format!("/mnt/riffcastle/upload-test/input-50g-{}.bin", run_id));
    let log_file = PathBuf::from(format!("/tmp/neverust-upload-50g-file-{}.log", run_id));
    let resp_file = PathBuf::from(format!("/tmp/neverust-upload-50g-file-{}.resp", run_id));

    fs::create_dir_all(&data_dir)?;

    // Sparse file: 50 GiB logical size.
    let f = File::create(&input_file)?;
    f.set_len(50 * GIB)?;
    println!("INPUT_FILE_CREATED={}", input_file.display());

    let log = File::create(&log_file)?;
    let log_err = log.try_clone()?;
    let child = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("start")
        .arg("--data-dir")
        .arg(data_dir.as_os_str())
        .arg("--listen-port")
        .arg(port_p2p.to_string())
        .arg("--disc-port")
        .arg(port_disc.to_string())
        .arg("--api-port")
        .arg(port_api.to_string())
        .arg("--log-level")
        .arg("warn")
        .arg("--bootstrap-node")
        .arg("/ip4/127.0.0.1/tcp/1")
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()?;
    let mut node = NodeGuard { child };

    let client = Client::builder()
        .timeout(Duration::from_secs(21600))
        .build()?;
    wait_for_health(&client, port_api, 120)?;

    println!("RUN_ID={}", run_id);
    println!("DATA_DIR={}", data_dir.display());
    println!("INPUT_FILE={}", input_file.display());
    println!("LOG_FILE={}", log_file.display());
    println!("RESP_FILE={}", resp_file.display());

    let start = Instant::now();
    let body_file = File::open(&input_file)?;
    let file_len = body_file.metadata()?.len();
    let resp = client
        .post(format!(
            "http://127.0.0.1:{}/api/archivist/v1/data",
            port_api
        ))
        .header("content-type", "application/octet-stream")
        .header("content-length", file_len.to_string())
        .body(body_file)
        .send();
    let elapsed = start.elapsed().as_secs();

    match resp {
        Ok(response) => {
            let status = response.status();
            let text = response.text().unwrap_or_else(|_| String::new());
            fs::write(&resp_file, text.as_bytes())?;
            println!("CURL_RC=0");
            println!(
                "NODE_ALIVE={}",
                if node.child.try_wait()?.is_none() {
                    "yes"
                } else {
                    "no"
                }
            );
            println!("ELAPSED_SEC={}", elapsed);
            println!("HTTP_CODE={}", status.as_u16());
            println!("TIME_SEC={}", elapsed);
        }
        Err(e) => {
            println!("CURL_RC=2");
            println!(
                "NODE_ALIVE={}",
                if node.child.try_wait()?.is_none() {
                    "yes"
                } else {
                    "no"
                }
            );
            println!("ELAPSED_SEC={}", elapsed);
            println!("RESULT={}", e);
        }
    }

    println!("RESP_PREVIEW_BEGIN");
    if let Ok(mut f) = File::open(&resp_file) {
        let mut buf = vec![0u8; 240];
        let n = f.read(&mut buf)?;
        print!("{}", String::from_utf8_lossy(&buf[..n]));
    }
    println!();
    println!("RESP_PREVIEW_END");

    let blocks_dir = data_dir.join("blocks");
    if blocks_dir.exists() {
        let du = Command::new("du")
            .arg("-sh")
            .arg(&blocks_dir)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        let count = Command::new("sh")
            .arg("-lc")
            .arg(format!("find '{}' -type f | wc -l", blocks_dir.display()))
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        println!("BLOCKS_DIR_USAGE:");
        println!("{}", du);
        println!("BLOCKS_FILE_COUNT:");
        println!("{}", count);
    }

    let ls_input = Command::new("ls")
        .arg("-lh")
        .arg(&input_file)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let du_input = Command::new("du")
        .arg("-h")
        .arg(&input_file)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!("INPUT_FILE_USAGE:");
    println!("{}", ls_input);
    println!("{}", du_input);

    let df = Command::new("sh")
        .arg("-lc")
        .arg("df -h /mnt/riffcastle/upload-test | sed -n '1,2p'")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!("DISK_FREE:");
    println!("{}", df);

    println!("LOG_TAIL_BEGIN");
    print!("{}", tail_file(&log_file, 20000));
    println!("LOG_TAIL_END");

    Ok(())
}
