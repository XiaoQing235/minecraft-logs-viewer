"""Generate small synthetic fixtures. Python is a development tool only."""
from pathlib import Path
import gzip
import io
import tarfile
import warnings
import zipfile
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent / "tests" / "fixtures"
root.mkdir(parents=True, exist_ok=True)
text = "[12:34:56] [Server thread/INFO]: §a你好§lABC§bDEF\r\n[12:35:00] [Server thread/ERROR] [minecraft/Main]: synthetic failure\r\n\tat example.Test.run(Test.java:1)\r\nlast line"
data = text.encode("utf-8")
(root / "latest.log").write_bytes(data)
(root / "utf16.log").write_bytes(b"\xff\xfe" + text.encode("utf-16-le"))
(root / "gb18030.log").write_bytes(text.encode("gb18030"))
(root / "empty.log").write_bytes(b"")
compressed = gzip.compress(data, mtime=0)
(root / "sample.log.gz").write_bytes(compressed)
entries = {"latest.log": data, "logs/nested.log": data, "logs/compressed.log.gz": compressed,
           "logs/deep/ignored.log": b"ignored", "backup/logs/ignored.log": b"ignored", "../ignored.log": b"ignored"}
for method, name in [(zipfile.ZIP_STORED, "stored.zip"), (zipfile.ZIP_DEFLATED, "deflate.zip")]:
    with zipfile.ZipFile(root / name, "w", compression=method) as archive:
        for filename, content in entries.items():
            archive.writestr(filename, content)
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            archive.writestr("latest.log", b"second occurrence\n")
for mode, name in [("w", "sample.tar"), ("w:gz", "sample.tar.gz")]:
    with tarfile.open(root / name, mode) as archive:
        for filename, content in entries.items():
            info = tarfile.TarInfo(filename)
            info.size = len(content)
            archive.addfile(info, io.BytesIO(content))
        link = tarfile.TarInfo("link.log")
        link.type = tarfile.SYMTYPE
        link.linkname = "latest.log"
        archive.addfile(link)
if shutil.which("7z"):
    scratch = root.parent.parent / "build" / "fixtures"
    scratch.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=scratch) as temporary:
        folder = Path(temporary)
        (folder / "logs").mkdir()
        (folder / "latest.log").write_bytes(data)
        (folder / "logs/nested.log").write_bytes(data)
        (folder / "logs/compressed.log.gz").write_bytes(compressed)
        for method, filename in [("LZMA", "lzma.7z"), ("LZMA2", "lzma2.7z")]:
            subprocess.run(["7z", "a", "-t7z", "-m0=" + method, str(root / filename), "latest.log", "logs"], cwd=folder, check=True, stdout=subprocess.DEVNULL)
