# 测试样本

这里的 LOG、GZIP、TAR 与 ZIP 是 `scripts/generate-fixtures.py` 生成的人工文本，包括中文、Minecraft 控制字符、重复 ZIP 成员和超出归档匹配范围的成员。UTF-16 和 GB18030 文件包含相同文本。

测试不使用个人 Minecraft 日志。`.log.gz` 成员与 ZIP 根目录、直接 `logs/` 成员分别验证；更深目录、路径越界和符号链接必须被忽略。

LZMA 与 LZMA2 的 7z 样本由同一脚本调用 7-Zip 生成，包含相同人工日志。
