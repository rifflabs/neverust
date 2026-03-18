use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use libloading::{Library, Symbol};
use std::ffi::{c_char, c_void, CStr, CString};
use std::net::{TcpListener, UdpSocket};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tower::util::ServiceExt;

use libp2p::identity::Keypair;
use neverust_core::{api, Block, BlockStore, BoTgConfig, BoTgProtocol, Discovery, Metrics};

type FfiStart = unsafe extern "C" fn(
    data_dir: *const c_char,
    api_bindaddr: *const c_char,
    api_port: u16,
    disc_port: u16,
    listen_port: u16,
    bootstrap_sprs: *const c_char,
) -> *mut c_void;
type FfiPoll = unsafe extern "C" fn();
type FfiStop = unsafe extern "C" fn(node_ptr: *mut c_void) -> i32;
type FfiLastError = unsafe extern "C" fn(node_ptr: *mut c_void) -> *const c_char;

fn cstr_or_empty(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: ptr comes from Nim and is expected to be a valid null-terminated string.
    unsafe { CStr::from_ptr(ptr).to_string_lossy().to_string() }
}

fn free_tcp_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind tcp port")
        .local_addr()
        .expect("read tcp addr")
        .port()
}

fn free_udp_port() -> u16 {
    UdpSocket::bind(("127.0.0.1", 0))
        .expect("bind udp port")
        .local_addr()
        .expect("read udp addr")
        .port()
}

fn compile_archivist_ffi_shim() -> Result<PathBuf, String> {
    let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = core_dir
        .parent()
        .ok_or_else(|| "cannot resolve workspace root".to_string())?;
    let archivist_root = std::env::var("ARCHIVIST_NODE_FFI_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root.join("../archivist-node"));
    if !archivist_root.exists() {
        return Err(format!(
            "ARCHIVIST_NODE_FFI_ROOT path does not exist: {}",
            archivist_root.display()
        ));
    }
    let source = core_dir.join("ffi/archivist_ffi.nim");
    let out_dir = workspace_root.join("target/archivist-ffi");
    let lib_path = out_dir.join("libarchivist_ffi.so");
    let cargo_home = out_dir.join("cargo-home");
    let nimcache_dir = out_dir.join("nimcache");
    let leopard_build_dir = nimcache_dir.join("vendor_leopard");

    std::fs::create_dir_all(&out_dir).map_err(|e| {
        format!(
            "failed to create shim output directory {}: {}",
            out_dir.display(),
            e
        )
    })?;
    std::fs::create_dir_all(&cargo_home).map_err(|e| {
        format!(
            "failed to create local cargo home {}: {}",
            cargo_home.display(),
            e
        )
    })?;
    if leopard_build_dir.exists() {
        std::fs::remove_dir_all(&leopard_build_dir).map_err(|e| {
            format!(
                "failed to clear stale Leopard build directory {}: {}",
                leopard_build_dir.display(),
                e
            )
        })?;
    }
    std::fs::create_dir_all(&leopard_build_dir).map_err(|e| {
        format!(
            "failed to create local Leopard build directory {}: {}",
            leopard_build_dir.display(),
            e
        )
    })?;
    let mut command = Command::new("nim");
    command
        .current_dir(&archivist_root)
        .env("CARGO_HOME", &cargo_home)
        .env("RUSTFLAGS", "-C relocation-model=pic")
        .arg("c")
        .arg("--app:lib")
        .arg(format!("--nimcache:{}", nimcache_dir.display()))
        .arg(format!("--path:{}", archivist_root.display()))
        .arg(format!("--out:{}", lib_path.display()));

    let vendor_nimble = archivist_root.join("vendor/nimble");
    if vendor_nimble.exists() {
        let entries = std::fs::read_dir(&vendor_nimble).map_err(|e| {
            format!(
                "failed to read vendored nimble directory {}: {}",
                vendor_nimble.display(),
                e
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("failed reading nimble entry: {}", e))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let src = path.join("src");
            let include_path = if src.is_dir() { src } else { path };
            command.arg(format!("--path:{}", include_path.display()));
        }
    }

    let status = command
        .arg(&source)
        .status()
        .map_err(|e| format!("failed to run nim compiler: {}", e))?;

    if !status.success() {
        return Err(format!(
            "nim failed compiling Archivist FFI shim with status {}",
            status
        ));
    }

    Ok(lib_path)
}

struct EmbeddedArchivistNode {
    api_port: u16,
    stop_tx: Option<mpsc::Sender<()>>,
    join_handle: Option<thread::JoinHandle<Result<(), String>>>,
    _data_dir: TempDir,
}

impl EmbeddedArchivistNode {
    fn start() -> Result<Self, String> {
        Self::start_with_bootstrap("")
    }

    fn start_with_bootstrap(bootstrap_sprs: &str) -> Result<Self, String> {
        let lib_path = compile_archivist_ffi_shim()?;
        let api_port = free_tcp_port();
        let disc_port = free_udp_port();
        let listen_port = free_tcp_port();
        let data_dir =
            tempfile::tempdir().map_err(|e| format!("failed to create temp dir: {}", e))?;

        let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), String>>(1);
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let data_dir_path = data_dir.path().to_path_buf();
        let bootstrap_sprs = bootstrap_sprs.to_string();

        let join_handle = thread::spawn(move || -> Result<(), String> {
            // SAFETY: Symbol loading and calls use known signatures from our shim.
            unsafe {
                let library = Library::new(&lib_path).map_err(|e| {
                    format!(
                        "failed to load Archivist FFI library {}: {}",
                        lib_path.display(),
                        e
                    )
                })?;

                let start: Symbol<FfiStart> = library
                    .get(b"archivist_ffi_start\0")
                    .map_err(|e| format!("missing archivist_ffi_start symbol: {}", e))?;
                let poll: Symbol<FfiPoll> = library
                    .get(b"archivist_ffi_poll\0")
                    .map_err(|e| format!("missing archivist_ffi_poll symbol: {}", e))?;
                let stop: Symbol<FfiStop> = library
                    .get(b"archivist_ffi_stop\0")
                    .map_err(|e| format!("missing archivist_ffi_stop symbol: {}", e))?;
                let last_error: Symbol<FfiLastError> =
                    library
                        .get(b"archivist_ffi_last_error\0")
                        .map_err(|e| format!("missing archivist_ffi_last_error symbol: {}", e))?;

                let data_dir_c = CString::new(
                    data_dir_path
                        .to_str()
                        .ok_or_else(|| {
                            format!(
                                "non-utf8 data dir path for Archivist node: {}",
                                data_dir_path.display()
                            )
                        })?
                        .as_bytes(),
                )
                .map_err(|e| format!("invalid data dir CString: {}", e))?;
                let bindaddr_c =
                    CString::new("127.0.0.1").map_err(|e| format!("bindaddr CString: {}", e))?;
                let bootstrap_c = CString::new(bootstrap_sprs.as_bytes())
                    .map_err(|e| format!("bootstrap CString: {}", e))?;

                let node_ptr = start(
                    data_dir_c.as_ptr(),
                    bindaddr_c.as_ptr(),
                    api_port,
                    disc_port,
                    listen_port,
                    bootstrap_c.as_ptr(),
                );

                if node_ptr.is_null() {
                    let err = cstr_or_empty(last_error(std::ptr::null_mut()));
                    let _ =
                        ready_tx.send(Err(format!("archivist_ffi_start returned null: {}", err)));
                    return Err(format!("archivist_ffi_start failed: {}", err));
                }

                if ready_tx.send(Ok(())).is_err() {
                    let _ = stop(node_ptr);
                    return Err("failed to signal Archivist startup to test thread".to_string());
                }

                loop {
                    match stop_rx.try_recv() {
                        Ok(_) | Err(TryRecvError::Disconnected) => break,
                        Err(TryRecvError::Empty) => {}
                    }
                    poll();
                    thread::sleep(Duration::from_millis(2));
                }

                let rc = stop(node_ptr);
                if rc != 0 {
                    let err = cstr_or_empty(last_error(node_ptr));
                    return Err(format!("archivist_ffi_stop failed: {}", err));
                }
            }

            Ok(())
        });

        match ready_rx.recv_timeout(Duration::from_secs(120)) {
            Ok(Ok(())) => {}
            Ok(Err(err)) => return Err(err),
            Err(e) => {
                return Err(format!(
                    "timed out waiting for embedded Archivist startup signal: {}",
                    e
                ));
            }
        }

        Ok(Self {
            api_port,
            stop_tx: Some(stop_tx),
            join_handle: Some(join_handle),
            _data_dir: data_dir,
        })
    }

    fn api_base_url(&self) -> String {
        format!("http://127.0.0.1:{}/api/archivist/v1", self.api_port)
    }

    async fn wait_ready(&self) -> Result<(), String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| format!("failed to build readiness client: {}", e))?;

        let deadline = Instant::now() + Duration::from_secs(30);
        let url = format!("{}/peerid", self.api_base_url());
        loop {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => return Ok(()),
                _ => {
                    if Instant::now() > deadline {
                        return Err(format!(
                            "embedded Archivist API did not become ready at {}",
                            url
                        ));
                    }
                    tokio::time::sleep(Duration::from_millis(150)).await;
                }
            }
        }
    }

    async fn wait_spr_ready(&self) -> Result<String, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| format!("failed to build spr readiness client: {}", e))?;

        let deadline = Instant::now() + Duration::from_secs(30);
        let url = format!("{}/spr", self.api_base_url());
        loop {
            match client
                .get(&url)
                .header("accept", "text/plain")
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    let body = resp
                        .text()
                        .await
                        .map_err(|e| format!("failed to read SPR response body: {}", e))?;
                    let spr = body.trim().to_string();
                    if spr.starts_with("spr:") {
                        return Ok(spr);
                    }
                }
                _ => {}
            }

            if Instant::now() > deadline {
                return Err(format!(
                    "embedded Archivist SPR endpoint did not become ready at {}",
                    url
                ));
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }

    fn stop(mut self) -> Result<(), String> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join_handle.take() {
            match join.join() {
                Ok(result) => result,
                Err(_) => Err("embedded Archivist thread panicked".to_string()),
            }
        } else {
            Ok(())
        }
    }
}

impl Drop for EmbeddedArchivistNode {
    fn drop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join_handle.take() {
            let _ = join.join();
        }
    }
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        // SAFETY: test-scoped env mutation for deterministic integration setup.
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // SAFETY: restores process env to previous state at test teardown.
        unsafe {
            match &self.previous {
                Some(prev) => std::env::set_var(self.key, prev),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

fn neverust_test_app() -> axum::Router {
    let block_store = Arc::new(BlockStore::new());
    let metrics = Metrics::new();
    let botg = Arc::new(BoTgProtocol::new(BoTgConfig::default()));
    let keypair = Arc::new(Keypair::generate_ed25519());
    let peer_id = "12D3KooWArchivistInteropTest".to_string();
    let listen_addrs = Arc::new(RwLock::new(vec!["/ip4/127.0.0.1/tcp/8070"
        .parse()
        .expect("valid neverust listen address")]));

    api::create_router(block_store, metrics, peer_id, botg, keypair, listen_addrs)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ffi_embedded_archivist_upload_then_neverust_retrieve() {
    let archivist = EmbeddedArchivistNode::start().expect("start embedded Archivist");
    archivist
        .wait_ready()
        .await
        .expect("embedded Archivist API ready");

    let payload = b"ffi-interop-payload-neverust-archivist".repeat(2048);
    let upload_url = format!("{}/data", archivist.api_base_url());

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("build reqwest client");

    let upload_resp = client
        .post(&upload_url)
        .header("content-type", "application/octet-stream")
        .body(payload.clone())
        .send()
        .await
        .expect("upload request succeeds");
    assert!(
        upload_resp.status().is_success(),
        "upload failed with HTTP {}",
        upload_resp.status()
    );

    let manifest_cid = upload_resp
        .text()
        .await
        .expect("upload response body")
        .trim()
        .to_string();
    assert!(
        !manifest_cid.is_empty(),
        "upload returned empty manifest CID"
    );

    let _fallback_env = EnvVarGuard::set(
        "NEVERUST_HTTP_FALLBACK_PEERS",
        &format!("http://127.0.0.1:{}", archivist.api_port),
    );

    let app = neverust_test_app();
    let request = Request::builder()
        .uri(format!(
            "/api/archivist/v1/data/{}/network/stream",
            manifest_cid
        ))
        .body(Body::empty())
        .expect("valid neverust network stream request");

    let response = app.oneshot(request).await.expect("neverust response");
    assert_eq!(response.status(), StatusCode::OK);

    let downloaded = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("downloaded bytes");
    assert_eq!(downloaded.as_ref(), payload.as_slice());

    archivist.stop().expect("stop embedded Archivist");
}

fn reserve_udp_port_owned() -> (u16, UdpSocket) {
    let socket = UdpSocket::bind(("127.0.0.1", 0)).expect("bind owned udp port");
    let port = socket
        .local_addr()
        .expect("read owned udp addr")
        .port();
    (port, socket)
}

#[tokio::test]
#[ignore]
async fn ffi_embedded_archivist_discv5_provide_roundtrip() {
    let archivist = EmbeddedArchivistNode::start().expect("start embedded Archivist");
    archivist
        .wait_ready()
        .await
        .expect("embedded Archivist API ready");
    let archivist_spr = archivist
        .wait_spr_ready()
        .await
        .expect("embedded Archivist SPR ready");

    let (port1, socket1) = reserve_udp_port_owned();
    let (port2, socket2) = reserve_udp_port_owned();

    let key1 = Keypair::generate_secp256k1();
    let key2 = Keypair::generate_secp256k1();

    drop(socket1);
    drop(socket2);

    let disc1 = Arc::new(
        Discovery::new(
            &key1,
            format!("127.0.0.1:{port1}")
                .parse()
                .expect("socket addr 1"),
            vec!["/ip4/127.0.0.1/tcp/28070".to_string()],
            vec![archivist_spr.clone()],
        )
        .await
        .expect("create discovery 1"),
    );
    let disc2 = Arc::new(
        Discovery::new(
            &key2,
            format!("127.0.0.1:{port2}")
                .parse()
                .expect("socket addr 2"),
            vec!["/ip4/127.0.0.1/tcp/28071".to_string()],
            vec![archivist_spr],
        )
        .await
        .expect("create discovery 2"),
    );

    let run1 = tokio::spawn(disc1.clone().run());
    let run2 = tokio::spawn(disc2.clone().run());

    tokio::time::sleep(Duration::from_secs(3)).await;

    let cid = Block::new_sha256(b"ffi-archivist-discv5-provide-proof".to_vec())
        .expect("create test block")
        .cid;

    disc1.provide(&cid).await.expect("provide cid");

    let providers = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            match disc2.find(&cid).await {
                Ok(providers) if !providers.is_empty() => return providers,
                _ => tokio::time::sleep(Duration::from_millis(500)).await,
            }
        }
    })
    .await
    .expect("timed out waiting for embedded Archivist providers");

    assert!(
        !providers.is_empty(),
        "embedded Archivist did not return any provider records for CID {}",
        cid
    );

    run1.abort();
    run2.abort();
    archivist.stop().expect("stop embedded Archivist");
}
