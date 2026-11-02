//! 应用内更新（macOS）：版本来源只有 GitHub Releases 上的 `latest.json`（由发布工作流生成并签名，见 docs/RELEASING.md）。
//! 检查、下载、安装都走 tauri-plugin-updater；这里包成三条命令，前端只管显示：
//! - `update_check`：问一次有没有新版本，有就记下来并把版本号、更新说明给前端；
//! - `update_install`：下载并原地替换 .app（进度经 "update-progress" 事件报出）；
//! - `update_restart`：前端把项目写完之后再重启，不然防抖里的改动会丢。
//! 离线、GitHub 不通、清单签名不对：命令返回 Err，前端安静地当作"没有更新"，不弹错。

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// 上一次检查发现的更新（`update_install` 装的就是它）。
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

/// 给前端看的新版本信息。
#[derive(Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub current: String,
    /// 更新说明（latest.json 的 `notes`，发布时从 CHANGELOG 摘的那一节）
    pub notes: Option<String>,
    pub date: Option<String>,
}

/// 下载进度：已下载 / 总字节数（总数未知时为 null）。
#[derive(Serialize, Clone)]
struct Progress {
    done: u64,
    total: Option<u64>,
}

fn pending(app: &AppHandle) -> tauri::State<'_, Pending> {
    app.state::<Pending>()
}

/// 检查一次。`Ok(None)` 是已经最新；`Err` 是没查成（离线、GitHub 不通、清单损坏），前端不区分。
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    #[allow(unused_mut)]
    let mut builder = app.updater_builder().timeout(std::time::Duration::from_secs(20));
    // 只有调试构建可以把清单指到别处（本地假的 latest.json，用来走通"发现 → 下载 → 安装"）；正式包只认 tauri.conf.json 里的地址
    #[cfg(debug_assertions)]
    if let Ok(ep) = std::env::var("FG_UPDATE_ENDPOINT") {
        let url = tauri::Url::parse(&ep).map_err(|e| e.to_string())?;
        builder = builder.endpoints(vec![url]).map_err(|e| e.to_string())?;
    }
    let updater = builder.build().map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| e.to_string())?;
    let info = found.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current: u.current_version.clone(),
        notes: u.body.clone().filter(|s| !s.trim().is_empty()),
        date: u.date.and_then(|d| d.format(&time::format_description::well_known::Rfc3339).ok()),
    });
    *pending(&app).0.lock().unwrap_or_else(|e| e.into_inner()) = found;
    Ok(info)
}

/// 下载并安装上一次检查到的更新。装完 .app 已经换成新的，但正在跑的还是旧进程：前端随后调 `update_restart`。
#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    let update = pending(&app).0.lock().unwrap_or_else(|e| e.into_inner()).clone().ok_or_else(|| "no update checked".to_string())?;
    let mut done: u64 = 0;
    let progress = app.clone();
    let finished = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                done += chunk as u64;
                let _ = progress.emit("update-progress", Progress { done, total });
            },
            move || {
                let _ = finished.emit("update-downloaded", ());
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 重启到新版本。前端先把项目写完（和退出前的冲刷一样）再调这里。
#[tauri::command]
pub fn update_restart(app: AppHandle) {
    app.restart();
}
