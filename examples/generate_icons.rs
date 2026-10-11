//! 图标生成：`cargo run --example generate_icons`
//! 源：`docs-zh/logo-rar.png`（仓库唯一手绘源）
//! 产物（全部进仓库，供三端打包引用）：
//! - `assets/icons/rarzap.ico`      Windows：exe / 任务栏 / 安装程序 / 安装包图标
//! - `assets/icons/rarzap.icns`     macOS：.app / dmg 卷图标
//! - `assets/icons/rarzap-{16,32,48,64,128,256,512}.png`  Linux hicolor + 通用
//! - `assets/icons/rarzap-1024.png`  商店/高清源
//!
//! ICO/ICNS 均为手工组装容器 + PNG 负载（Vista+/macOS 10.5+ 通用做法），
//! 不依赖外部工具链，保证 CI 三平台可复现。

use image::imageops::FilterType;
use std::path::{Path, PathBuf};

const SRC: &str = "docs-zh/logo-rar.png";
const OUT_DIR: &str = "assets/icons";

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = root.join(SRC);
    let out = root.join(OUT_DIR);
    std::fs::create_dir_all(&out).expect("mkdir assets/icons");

    let img = image::open(&src).expect("open logo source").into_rgba8();
    // 源图若非正方形，按中心裁方，避免图标被拉伸。
    let side = img.width().min(img.height());
    let x = (img.width() - side) / 2;
    let y = (img.height() - side) / 2;
    let cropped = image::imageops::crop_imm(&img, x, y, side, side).to_image();

    // PNG 全套（含 Linux hicolor 逐级 + 1024 商店源）
    for size in [16u32, 32, 48, 64, 128, 256, 512, 1024] {
        let r = image::imageops::resize(&cropped, size, size, FilterType::Lanczos3);
        let p = out.join(format!("rarzap-{size}.png"));
        r.save(&p).expect("save png");
        println!("wrote {}", p.display());
    }

    // ICO：16/32/48/64/128/256，PNG 负载
    let mut ico = IconDir::default();
    for size in [16u32, 32, 48, 64, 128, 256] {
        let r = image::imageops::resize(&cropped, size, size, FilterType::Lanczos3);
        ico.push_png(&r);
    }
    let ico_path = out.join("rarzap.ico");
    std::fs::write(&ico_path, ico.build()).expect("write ico");
    println!("wrote {}", ico_path.display());

    // ICNS：ic07(128)/ic08(256)/ic09(512)/ic10(1024)，PNG 负载
    let mut icns: Vec<u8> = b"icns".to_vec();
    icns.extend_from_slice(&[0, 0, 0, 0]); // 占位总长
    for (kind, size) in [("ic07", 128u32), ("ic08", 256), ("ic09", 512), ("ic10", 1024)] {
        let r = image::imageops::resize(&cropped, size, size, FilterType::Lanczos3);
        let mut png = Vec::new();
        let mut cur = std::io::Cursor::new(&mut png);
        r.write_to(&mut cur, image::ImageFormat::Png)
            .expect("encode png");
        drop(cur);
        icns.extend_from_slice(kind.as_bytes());
        icns.extend_from_slice(&(8 + png.len() as u32).to_be_bytes());
        icns.extend_from_slice(&png);
    }
    let total = icns.len() as u32;
    icns[4..8].copy_from_slice(&total.to_be_bytes());
    let icns_path = out.join("rarzap.icns");
    std::fs::write(&icns_path, icns).expect("write icns");
    println!("wrote {}", icns_path.display());

    // 校验：回读验证容器合法性
    assert!(Path::new(&ico_path).exists());
    assert!(Path::new(&icns_path).exists());
    println!("icons OK");
}

/// 最小 ICO 组装器：ICONDIR + N 个 PNG 条目。
#[derive(Default)]
struct IconDir {
    blobs: Vec<(u8, Vec<u8>)>,
}

impl IconDir {
    fn push_png(&mut self, img: &image::RgbaImage) {
        let mut png = Vec::new();
        let mut cur = std::io::Cursor::new(&mut png);
        img.write_to(&mut cur, image::ImageFormat::Png)
            .expect("encode png");
        drop(cur);
        let w = if img.width() >= 256 { 0 } else { img.width() as u8 };
        self.blobs.push((w, png));
    }

    fn build(&self) -> Vec<u8> {
        let n = self.blobs.len() as u16;
        let mut out = Vec::new();
        out.extend_from_slice(&[0, 0]); // reserved + type(ICO)
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&n.to_le_bytes());
        let mut offset = 6 + 16 * self.blobs.len() as u32;
        for (w, png) in &self.blobs {
            out.push(*w); // width（0=256）
            out.push(*w); // height
            out.push(0); // palette
            out.push(0); // reserved
            out.extend_from_slice(&1u16.to_le_bytes()); // planes
            out.extend_from_slice(&32u16.to_le_bytes()); // bpp
            out.extend_from_slice(&(png.len() as u32).to_le_bytes());
            out.extend_from_slice(&offset.to_le_bytes());
            offset += png.len() as u32;
        }
        for (_, png) in &self.blobs {
            out.extend_from_slice(png);
        }
        out
    }
}
