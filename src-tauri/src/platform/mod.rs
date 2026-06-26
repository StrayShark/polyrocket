//! L5 —— 平台层。
//!
//! 该层是 polyrocket 中**唯一**直接与宿主操作系统交互的地方。
//! 其他所有层(L1-L4)都通过这里访问系统资源。
//!
//! 模块
//! -------
//! - [`keyring`] —— macOS Keychain / Windows Credential Manager / Linux
//!   Secret Service。`keyring` crate 处理跨平台抽象,
//!   因此每个 OS 的代码只是一个工厂函数 + 服务名。
//! - [`env`] —— 读取 `POLYROCKET_*` 环境变量 + dev 模式 `.env` 同步。
//! - [`paths`] —— 解析应用数据目录、数据库路径、日志路径。
//!
//! 分层规则
//! -----------
//! - L5 不得依赖 L1、L2、L3 或 L4(否则会形成循环)。
//! - L5 模块之间可以互相引用(env 使用 keyring,paths 是叶子模块)。
//! - 所有密钥进出进程**只能**通过 `keyring::*`。
//!
//! 完整模块图见 `docs/overview.md` §2.1。

pub mod env;
pub mod keyring;
pub mod paths;

// 重新导出高层 API 表面,调用方可以 `use crate::platform::*;`
// 而无需深入三层。
pub use env::{maybe_load_dev_env, parse_env_file};
pub use keyring::{
    delete_key, delete_provider_keys, get_key, has_key, set_key,
    llm_alias, pm_api_alias, pm_passphrase_alias, pm_secret_alias, wallet_alias,
};
pub use paths::{app_data_dir, db_path, log_dir, sqlite_url};
