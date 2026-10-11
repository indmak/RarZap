//! 无头转换示例（验证 CORE 全链路 + 进度回调，不启动 UI）。
//!
//! 用法：
//!   cargo run --example convert -- <input.rar> [output_dir]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use rarzap::core::Phase;
use rarzap::free::{SingleRequest, run_single_with_progress};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let Some(input) = args.next() else {
        eprintln!("usage: convert <input.rar> [output_dir]");
        std::process::exit(2);
    };
    let output_dir = args.next().map(PathBuf::from);

    let req = SingleRequest::new(PathBuf::from(input), output_dir);
    let never = AtomicBool::new(false);
    let started = std::time::Instant::now();

    let mut last_line = String::new();
    let zip = run_single_with_progress(
        &req,
        Some(&|| never.load(Ordering::SeqCst)),
        &mut |p| {
            let phase = match p.phase {
                Phase::Extracting => "extract",
                Phase::Packing => "pack",
            };
            let line = format!(
                "[{phase}] {:>5.1}%  {}/{}  {}",
                p.percent * 100.0,
                p.files_done,
                p.files_total,
                p.current_file
            );
            // 回车覆盖同一行，终端不刷屏。
            if line != last_line {
                print!("\r{:<100}", line);
                use std::io::Write as _;
                let _ = std::io::stdout().flush();
                last_line = line;
            }
        },
    )?;

    println!("\nOK -> {}", zip.display());
    println!("elapsed: {:.2}s", started.elapsed().as_secs_f32());
    Ok(())
}
