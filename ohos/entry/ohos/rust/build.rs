fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");

    // 配置 NDK 路径（需要根据实际环境调整）
    let ohos_ndk_path = std::env::var("HARMONYOS_NDK_PATH")
        .expect("请设置 HARMONYOS_NDK_PATH 环境变量");

    // LLVM 库搜索路径（包含 libc++.a 和 libunwind.a）
    println!("cargo:rustc-link-search={}/llvm/lib/aarch64-linux-ohos", ohos_ndk_path);

    // Sysroot 库搜索路径（包含 libace_napi.z.so 和 libc.so）
    println!("cargo:rustc-link-search={}/sysroot/usr/lib/aarch64-linux-ohos", ohos_ndk_path);

    // 链接必要的系统库
    println!("cargo:rustc-link-lib=static=c++");
    println!("cargo:rustc-link-lib=static=unwind");
    println!("cargo:rustc-link-lib=dylib=ace_napi.z");
    println!("cargo:rustc-link-lib=dylib=c");
}
