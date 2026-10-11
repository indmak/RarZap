//! Windows 构建：把 assets/icons/rarzap.ico 链进 exe（窗口左上/任务栏图标来源），
//! 并写入四段文件版本。仅 Windows target 生效；其它平台直接返回。
//!
//! 该资源只在启用 `winres` feature 时注入（默认开启）。商业二进制作为独立 crate
//! 依赖本库时会关闭该 feature，避免两份 VERSION/ICON 资源冲突（CVT1100）。

fn main() {
    // 被依赖且关闭了 feature 时：不注入资源。
    if std::env::var_os("CARGO_FEATURE_WINRES").is_none() {
        return;
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // 版本统一：Cargo `0.1.0` -> 文件版本 u64(0.1.0.0)；已是四段则原样用。
    let parts: Vec<u64> = env!("CARGO_PKG_VERSION")
        .split('.')
        .take(4)
        .map(|s| s.parse().unwrap_or(0))
        .collect();
    let mut p = [0u64; 4];
    for (i, v) in parts.into_iter().enumerate() {
        p[i] = v;
    }
    let ver_u64 = (p[0] << 48) | (p[1] << 32) | (p[2] << 16) | p[3];

    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/icons/rarzap.ico");
    res.set_version_info(winres::VersionInfo::PRODUCTVERSION, ver_u64);
    res.set_version_info(winres::VersionInfo::FILEVERSION, ver_u64);
    res.set("ProductName", "RarZap");
    res.set("FileDescription", "RarZap — RAR to ZIP converter");
    res.set("CompanyName", "RarZap");
    if let Err(e) = res.compile() {
        // rc.exe 缺失时（如精简工具链）不阻断构建：安装包阶段仍有图标。
        println!("cargo:warning=winres skipped: {e}");
    }
}
