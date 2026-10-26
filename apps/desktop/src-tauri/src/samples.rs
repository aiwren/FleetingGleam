//! 示例照片：应用自带的十张小照片（和 Android 的 assets/samples 同一批：落地页用的 Lorem Picsum，Unsplash 授权），
//! 编进应用里，第一次要用时复制到数据目录的 `samples/`，起始屏的"试试示例照片"拿它建一个项目。
//! 其中两组是同一张的轻微裁切 / 模糊变体，能看到"相似的归成一组、推荐一张"。不联网。

use tauri::AppHandle;

macro_rules! sample {
    ($n:literal) => {
        ($n, include_bytes!(concat!("../samples/", $n)) as &[u8])
    };
}

const FILES: &[(&str, &[u8])] = &[
    sample!("sample-01.jpg"),
    sample!("sample-01b.jpg"),
    sample!("sample-01c.jpg"),
    sample!("sample-02.jpg"),
    sample!("sample-03.jpg"),
    sample!("sample-05.jpg"),
    sample!("sample-06.jpg"),
    sample!("sample-07.jpg"),
    sample!("sample-07b.jpg"),
    sample!("sample-07c.jpg"),
];

/// 把示例照片放进 `dir`（已经在的不动，被删掉的补回来）。
fn materialize(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, bytes) in FILES {
        let f = dir.join(name);
        if std::fs::metadata(&f).map(|m| m.len() != bytes.len() as u64).unwrap_or(true) {
            std::fs::write(&f, bytes)?;
        }
    }
    Ok(())
}

/// 示例照片所在的文件夹（第一次调用时复制出来）。
#[tauri::command]
pub fn samples_folder(app: AppHandle) -> Result<String, String> {
    let dir = crate::data_dir(&app).join("samples");
    materialize(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    #[test]
    fn writes_every_sample_once() {
        let dir = std::env::temp_dir().join(format!("fg-samples-{}", std::process::id()));
        super::materialize(&dir).unwrap();
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), super::FILES.len());
        // 再来一次不出错，也不多出文件
        super::materialize(&dir).unwrap();
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), super::FILES.len());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
