//! RarZap 开源版入口（单文件 RAR -> ZIP）。
//! 全部逻辑在库里（`rarzap::run`），本文件只做转发，避免 bin/lib 两套模块树。

fn main() {
    rarzap::run();
}
