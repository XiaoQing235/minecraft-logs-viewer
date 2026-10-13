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

## 持续集成与发行包

`.github/workflows/build.yml` 包含四个本机架构任务：Windows x64、macOS arm64、macOS x64、Linux x64。各任务准备固定 baseline 的静态原生库，执行 Clippy、格式检查、行为测试、Release 构建、BetterLeaks 与打包。Windows / Linux 增加窗口 smoke，Linux 使用 Xvfb；macOS 原生窗口及输入保留为人工验收。

使用 `./scripts/package.ps1 -Platform windows-x64`，其他可选值为 `macos-arm64`、`macos-x64`、`linux-x64`。脚本从当前平台的 Release 产物组装 EXE、二进制或 APP，收集依赖许可，分别检查应用本体和 ZIP 小于 15,000,000 字节。输出 manifest 记录源提交、工作树是否有修改、字节数和 SHA-256。脚本拒绝覆盖已有 staging 和 ZIP；重复打包前可清理对应的 `dist` 生成物。

项目尚未配置远程仓库，当前没有远端 CI 执行结果。平台矩阵标签参照 [GitHub hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)。

## 测量与窗口检查

`examples/measure.rs` 使用生产读取、稀疏定位和搜索代码，接受人工生成日志路径。它记录首个索引批次、完整索引、1,000 次定位、无命中搜索、预取消检查及缓存量。它不测量 GUI 首屏和冷盘读取。

配置原生环境后执行 `cargo build --locked --release --example measure`，构建成功必须紧接 `./scripts/check-secrets.ps1`。运行示例：`./target/release/examples/measure ./build/measurements/100m.log`。Windows 后缀为 `.exe`。

窗口 smoke 使用人工样本：

```text
minecraft-logs-viewer ./tests/fixtures --smoke --screenshot build/window.bmp
minecraft-logs-viewer ./tests/fixtures --smoke --light --screenshot build/light.bmp
minecraft-logs-viewer ./tests/fixtures --smoke --smoke-query ABCDEF --smoke-save build/smoke-copy.log
```

测试模式跳过个人设置读写，3 秒后检查状态并退出。截图从窗口自身 ImageSurface 取得，包含应用实际绘制。`--window-size 1040x620` 可检查最小尺寸；smoke 不替代原生输入、文件选择器和剪贴板的人工验收。

## Git 记录

开发在 `dev` 分支进行，每个可验证阶段单独提交。提交前执行对应检查和 `git diff --check`；个人日志、运行配置、构建缓存与测试生成物不进入提交。未配置远程仓库时，提交保留在本地。
