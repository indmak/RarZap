//! 端到端转换测试（需要真实 RAR，默认忽略）。
//!
//! 运行：
//!   $env:RARZAP_TEST_RAR="C:\path\sample.rar"; cargo test --test convert_e2e -- --ignored --nocapture

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rarzap::free::{SingleRequest, run_single_with_progress};

fn fixture() -> Option<PathBuf> {
    std::env::var_os("RARZAP_TEST_RAR").map(PathBuf::from)
}

#[test]
#[ignore = "requires RARZAP_TEST_RAR pointing at a real .rar"]
fn converts_and_reports_progress() {
    let Some(rar) = fixture() else {
        eprintln!("RARZAP_TEST_RAR not set; skipping");
        return;
    };
    let out_dir = tempfile::tempdir().unwrap();
    let req = SingleRequest::new(rar, Some(out_dir.path().to_path_buf()));

    let events = Arc::new(AtomicUsize::new(0));
    let counter = events.clone();
    let never = AtomicBool::new(false);
    let zip = run_single_with_progress(
        &req,
        Some(&|| never.load(Ordering::SeqCst)),
        &mut |_p| {
            counter.fetch_add(1, Ordering::SeqCst);
        },
    )
    .expect("conversion should succeed");

    assert!(zip.exists(), "output zip should exist");
    assert!(std::fs::metadata(&zip).unwrap().len() > 0, "zip not empty");
    assert!(
        events.load(Ordering::SeqCst) > 0,
        "expected progress callbacks"
    );
}

#[test]
#[ignore = "requires RARZAP_TEST_RAR pointing at a real .rar"]
fn cancel_leaves_no_partial_output() {
    let Some(rar) = fixture() else {
        eprintln!("RARZAP_TEST_RAR not set; skipping");
        return;
    };
    let out_dir = tempfile::tempdir().unwrap();
    let req = SingleRequest::new(rar, Some(out_dir.path().to_path_buf()));
    let zip_path = rarzap::free::plan_single(&req).unwrap();

    // 首次进度回调后请求取消：后续取消检查点应立即中止并清理半成品。
    let flag = Arc::new(AtomicBool::new(false));
    let probe = flag.clone();
    let res = run_single_with_progress(&req, Some(&|| probe.load(Ordering::SeqCst)), &mut |_p| {
        flag.store(true, Ordering::SeqCst);
    });

    let err = res.expect_err("cancelled run should error");
    let chain = format!("{err:#}");
    assert!(
        chain.contains("cancelled"),
        "unexpected error: {chain}"
    );
    assert!(!zip_path.exists(), "no partial output should remain");
}
