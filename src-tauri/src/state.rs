use std::path::PathBuf;
use std::process::Child;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread::JoinHandle as ThreadJoinHandle;

use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::auth::PendingOauthLogin;
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

pub(crate) struct OauthCallbackListenerHandle {
    pub(crate) shutdown_tx: Option<Sender<()>>,
    pub(crate) task: Option<ThreadJoinHandle<()>>,
}

/// 全局运行态：
/// - `store_lock` 保证账号存储读写的串行化。
/// - `pending_oauth_login` 维护当前 OAuth 授权会话。
/// - `oauth_listener` 维护本地 OAuth 回调监听线程。
/// - `api_proxy` 维护本地 API 反代服务的生命周期与状态。
/// - `cloudflared` 维护公网隧道进程与当前状态。
pub(crate) struct AppState {
    pub(crate) store_lock: Arc<Mutex<()>>,
    pub(crate) oauth_flow_lock: Arc<Mutex<()>>,
    pub(crate) pending_oauth_login: Mutex<Option<PendingOauthLogin>>,
    pub(crate) oauth_listener: Mutex<Option<OauthCallbackListenerHandle>>,
    pub(crate) api_proxy: Mutex<Option<ApiProxyRuntimeHandle>>,
    pub(crate) cloudflared: Mutex<Option<CloudflaredRuntimeHandle>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            store_lock: Arc::new(Mutex::new(())),
            oauth_flow_lock: Arc::new(Mutex::new(())),
            pending_oauth_login: Mutex::new(None),
            oauth_listener: Mutex::new(None),
            api_proxy: Mutex::new(None),
            cloudflared: Mutex::new(None),
        }
    }
}
