use rarzap::core::{RarzapPlugin, default_zip_name_for, plugin::JobSpec};
use rarzap::free::{SingleRequest, is_rar_path, plan_single};
use std::path::{Path, PathBuf};

#[test]
fn zip_name_next_to_source() {
    let rar = Path::new("/tmp/archive_01.rar");
    assert_eq!(
        default_zip_name_for(rar, None),
        Path::new("/tmp/archive_01.zip")
    );
}

#[test]
fn zip_name_into_output_dir() {
    let rar = Path::new("/tmp/a.rar");
    let out = Path::new("/tmp/out");
    assert_eq!(
        default_zip_name_for(rar, Some(out)),
        Path::new("/tmp/out/a.zip")
    );
}

#[test]
fn free_rejects_non_rar() {
    assert!(is_rar_path(Path::new("a.rar")));
    assert!(!is_rar_path(Path::new("a.zip")));
    let req = SingleRequest::new(PathBuf::from("a.zip"), None);
    assert!(plan_single(&req).is_err());
}

#[test]
fn free_plans_single_target() {
    let req = SingleRequest::new(PathBuf::from("/tmp/a.rar"), None);
    assert_eq!(plan_single(&req).unwrap(), PathBuf::from("/tmp/a.zip"));
}

struct OssPlugin;
impl RarzapPlugin for OssPlugin {
    fn info(&self) -> rarzap::core::PluginInfo {
        rarzap::core::PluginInfo {
            id: "test",
            name: "test",
            version: "0.0.0",
        }
    }
}

#[test]
fn plugin_default_uses_core_single() {
    // 默认实现直通 CORE：缺失源文件应报错而非 panic。
    let p = OssPlugin;
    let spec = JobSpec::new(PathBuf::from("/nope/missing.rar"), PathBuf::from("/tmp/o.zip"));
    assert!(p.convert_single(&spec, None).is_err());
}
