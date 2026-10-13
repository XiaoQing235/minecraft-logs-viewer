# 开发与验证

应用使用 Rust 2024 edition，最低 Rust 1.88。Rust 依赖固定在 Cargo.lock；原生依赖由 vcpkg.json 的 baseline 固定，FLTK 从固定版本 crate 内源码构建。

## Windows

需要 PowerShell 7、Rust MSVC 工具链、Visual Studio C++ Build Tools、Git 和 BetterLeaks。首次准备归档原生库并构建：

```powershell
./scripts/build.ps1 -Prepare -Release
```

后续可运行 `-Action test`、`-Action clippy` 和 `-Action fmt`。成功构建与测试后脚本执行脱敏的 BetterLeaks 工作树扫描。编译产物位于 `target/release/`，原生依赖位于被忽略的 `build/`。

Windows 的 libarchive、zlib、liblzma 和 bzip2 使用静态链接，MSVC 运行库采用系统运行库。当前 baseline 对应 libarchive 3.8.9、zlib 1.3.2、liblzma 5.8.4、bzip2 1.0.8。`check-secrets.ps1` 扫描当前工作树中全部已跟踪文件与未被忽略的新文件，包含本项目源码、脚本和配置；构建输出和下载的第三方源码位于忽略目录。

## macOS 与 Linux

使用相同脚本和本机架构 vcpkg triplet。macOS 需要 Xcode Command Line Tools、CMake、pkg-config 和 PowerShell。Linux 还需要 C++、Ninja、X11、Xft、Pango 和 Cairo 开发包。首轮 Linux 基线为 Ubuntu 22.04。

各平台的编译、运行和体积必须分别验证；本地 Windows 构建不代表其他目标已通过。

## Git 记录

开发在 `dev` 分支进行，每个可验证阶段单独提交。提交前执行对应检查和 `git diff --check`；个人日志、运行配置、构建缓存与测试生成物不进入提交。未配置远程仓库时，提交保留在本地。
