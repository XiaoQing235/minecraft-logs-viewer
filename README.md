# minecraft-logs-viewer

用于在单个窗口中浏览、搜索和编辑 Minecraft 日志副本的本地桌面工具。

应用代码使用 Rust，界面使用 FLTK。Windows 已有可运行的首版；构建流程包含 Windows x64、macOS arm64 / x64 和 Linux x64。每个平台的下载包及应用本体设有 15,000,000 字节上限。

## 功能范围

- 一个窗口中的来源树、虚拟日志表格、搜索结果及多个副本编辑标签页；深浅主题、系统字体和阅读字号。
- 普通 `.log`、`.log.gz`、TAR / TAR.GZ / TGZ、ZIP、7z、未加密 RAR4 / RAR5；读取归档根目录与根目录下 `logs/` 的直接子日志，包括其中的 `.log.gz`。
- 当前日志、选中来源和整个目录三种搜索范围；子串、逐行正则、大小写选项及级别、线程、logger、时间过滤。
- 按隐藏 `§` 字符后的可见文字搜索，命中映射回原文；保留未知行，识别常见 Minecraft / Log4j 前缀。
- 原版 Java `§0–§f`、`§k/l/m/n/o/r` 显示与原文切换。混淆仅作为预览，复制及编辑保留真实字符。
- BOM、UTF-8 样本校验、可能的 UTF-16 和其他编码检测；手动切换编码。编码说明的悬浮提示显示判断依据，解码错误有明确提示。
- 全文或片段副本编辑、撤销 / 重做、查找 / 替换、格式预览及另存；输出编码、BOM、换行和末尾换行可选择。
- `latest.log` 实时跟随及暂停；追加、截断、轮转和多字节字符边界处理，只显示已完成的行。
- 阅读位置、最近目录、书签、窗口大小、主题和字号保存在本机；可清理查看缓存。

## 使用

打开程序后选择日志文件夹，在来源树中选取普通日志或归档成员。可启用“包含子目录”；归档内的读取范围固定为根目录与 `logs/` 一层。归档成员同名时按物理序号分别展示。

查看器提供行号跳转、选中行复制和编码切换。需要修改时创建“编辑副本”或“片段副本”，在独立标签页另存。原件与归档保持只读，保存会拒绝覆盖已发现的来源文件及其硬链接，并检查已有目标是否被外部修改。

也可从命令行传入目录：

```text
minecraft-logs-viewer ./.minecraft/logs
```

## 范围与运行条件

归档示例 `backup.zip/latest.log` 与 `backup.zip/logs/latest.log` 均可读取。更深目录、归档内部的另一份归档、加密归档与分卷归档未作为首版支持目标。

Windows 使用系统 DLL、UCRT 及 Visual C++ v14 x64 运行库；运行库版本须不低于所用 MSVC Build Tools，本次为 14.51。运行库要求参照 [Microsoft 官方说明](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)。FLTK、libarchive 和归档解码库静态链接。macOS 与 Linux 的构建、体积、原生输入和界面运行仍需对应平台验证；Linux 使用系统 X11、Pango、Cairo 等桌面组件。具体本机证据见验收记录。

副本上限为 20 MiB；单个解码 / 解压缓存上限 1 GiB，总缓存上限 2 GiB，单行上限 32 MiB。搜索结果上限 10,000 个命中行，预览每行最多 1,024 字节；完整搜索使用原文。超限、取消、部分失败和完成状态分别报告。表格显示限制不会改变复制得到的内容。

异常栈当前保留逐行显示；折叠、独立长行详情和带来源 / 行号的结构化结果导出仍在后续计划中。读取缓存由进程拥有，正常退出清理；异常结束留下的临时缓存暂需从系统临时目录清理。

## 项目文档

- [架构与实施计划](docs/architecture-and-plan.md)
- [开发、构建与打包](docs/development.md)
- [首版验收记录](docs/implementation-status.md)

## 开发

在 `dev` 分支按功能阶段提交。依赖锁定于 Cargo.lock 与 vcpkg baseline；首次构建使用 `./scripts/build.ps1 -Prepare -Release`，随后使用 `./scripts/package.ps1 -Platform windows-x64` 打包。每次成功构建后执行脱敏的 BetterLeaks 工作树扫描。
