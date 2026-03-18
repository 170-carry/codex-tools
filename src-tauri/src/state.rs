use std::path::PathBuf;
use std::process::Child;
use std::sync::Arc;
use std::sync::RwLock;

use serde_json::Value;
use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::models::CloudflaredTunnelMode;

#[derive(Debug, Default, Clone)]
pub(crate) struct ApiProxyRuntimeSnapshot {
    pub(crate) active_account_key: Option<String>,
    pub(crate) active_account_id: Option<String>,
    pub(crate) active_account_label: Option<String>,
    pub(crate) last_error: Option<String>,
}

pub(crate) struct ApiProxyRuntimeHandle {
    pub(crate) port: u16,
    pub(crate) api_key: Arc<RwLock<String>>,
    pub(crate) shutdown_tx: Option<oneshot::Sender<()>>,
    pub(crate) task: JoinHandle<()>,
    pub(crate) shared: Arc<Mutex<ApiProxyRuntimeSnapshot>>,
}

pub(crate) struct CloudflaredRuntimeHandle {
    pub(crate) mode: CloudflaredTunnelMode,
    pub(crate) use_http2: bool,
    pub(crate) public_url: Option<String>,
    pub(crate) custom_hostname: Option<String>,
    pub(crate) last_error: Option<String>,
    pub(crate) cleanup_api_token: Option<String>,
    pub(crate) cleanup_account_id: Option<String>,
    pub(crate) cleanup_tunnel_id: Option<String>,
    pub(crate) log_path: PathBuf,
    pub(crate) child: Child,
}

/// 全局运行态：
/// - `store_lock` 保证账号存储读写的串行化。
/// - `add_flow_auth_backup` 用于“添加账号”流程前后的 auth.json 回滚。
/// - `api_proxy` 维护本地 API 反代服务的生命周期与状态。
/// - `cloudflared` 维护公网隧道进程与当前状态。
pub(crate) struct AppState {
    pub(crate) store_lock: Arc<Mutex<()>>,
    pub(crate) add_flow_auth_backup: Mutex<Option<Option<Value>>>,
    pub(crate) api_proxy: Mutex<Option<ApiProxyRuntimeHandle>>,
    pub(crate) cloudflared: Mutex<Option<CloudflaredRuntimeHandle>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            store_lock: Arc::new(Mutex::new(())),
            add_flow_auth_backup: Mutex::new(None),
            api_proxy: Mutex::new(None),
            cloudflared: Mutex::new(None),
        }
    }
}
