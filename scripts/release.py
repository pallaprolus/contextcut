#!/usr/bin/env python3
"""Package and smoke-test standalone binaries; generate checksums and a Homebrew formula."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-musl",
    "x86_64-unknown-linux-musl",
    "x86_64-pc-windows-msvc",
)


def version():
    value = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", value):
        raise ValueError("distribution expects a stable numeric version")
    return value


def filename(target):
    suffix = "zip" if "windows" in target else "tar.gz"
    return f"contextcut-{version()}-{target}.{suffix}"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def package(binary, target, out):
    out.mkdir(parents=True, exist_ok=True)
    destination = out / filename(target)
    executable = "contextcut.exe" if "windows" in target else "contextcut"
    entries = [(binary, executable), (ROOT / "LICENSE", "LICENSE"), (ROOT / "README.md", "README.md")]
    if destination.suffix == ".zip":
        with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as archive:
            for source, name in entries:
                archive.write(source, name)
    else:
        with tarfile.open(destination, "w:gz") as archive:
            for source, name in entries:
                info = archive.gettarinfo(source, arcname=name)
                info.mode = 0o755 if name == executable else 0o644
                with source.open("rb") as content:
                    archive.addfile(info, content)
    print(destination)


def smoke(archive):
    with tempfile.TemporaryDirectory(prefix="contextcut-smoke-") as directory:
        root = Path(directory)
        unpacked = root / "unpacked"
        unpacked.mkdir()
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as package_file:
                assert set(package_file.namelist()) == {"contextcut.exe", "LICENSE", "README.md"}
                package_file.extractall(unpacked)
        else:
            with tarfile.open(archive) as package_file:
                assert set(package_file.getnames()) == {"contextcut", "LICENSE", "README.md"}
                package_file.extractall(unpacked, filter="data")
        binary = unpacked / ("contextcut.exe" if os.name == "nt" else "contextcut")
        assert subprocess.check_output([str(binary), "--version"], text=True).strip() == f"contextcut {version()}"
        subprocess.run([str(binary), "review", "--help"], check=True, stdout=subprocess.DEVNULL)
        repo = root / "repo"
        repo.mkdir()
        source = repo / "example.py"
        source.write_text("answer = 41\n")
        def git(*args):
            subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True)
        git("init", "-q")
        git("add", ".")
        git("-c", "user.name=Release smoke test", "-c", "user.email=smoke@example.invalid", "commit", "-qm", "baseline")
        source.write_text("answer = 42\n")
        output = subprocess.run([str(binary), "review", str(repo), "--budget", "2k"], check=True, capture_output=True, text=True)
        assert "+answer = 42" in output.stdout
        assert "## Patch" in output.stdout
        assert "/ 2000 o200k_base tokens" in output.stderr
        # Confirm the distributable includes tokenizer data and stays independent of the checkout.
        packed = subprocess.run([str(binary), str(repo), "--budget", "2k"], cwd=root, check=True, capture_output=True, text=True)
        assert "answer = 42" in packed.stdout
    print(f"PASS: extracted {archive.name}; version, review, and packing work")


def manifest(out):
    hashes = {}
    for target in TARGETS:
        archive = out / filename(target)
        if not archive.is_file():
            raise ValueError(f"missing release archive: {archive}")
        hashes[target] = digest(archive)
    base = f"https://github.com/pallaprolus/contextcut/releases/download/v{version()}"
    formula = [
        "class Contextcut < Formula",
        '  desc "Prepare focused code context and change reviews for AI chats"',
        '  homepage "https://github.com/pallaprolus/contextcut"',
        f'  version "{version()}"',
        '  license "MIT"',
        "",
    ]
    for system, targets in [
        ("macos", ["aarch64-apple-darwin", "x86_64-apple-darwin"]),
        ("linux", ["aarch64-unknown-linux-musl", "x86_64-unknown-linux-musl"]),
    ]:
        formula.append(f"  on_{system} do")
        if system == "macos":
            formula.extend(['    depends_on macos: :big_sur', ""])
        for index, target in enumerate(targets):
            formula.extend([
                f"    on_{'arm' if index == 0 else 'intel'} do",
                f'      url "{base}/{filename(target)}"',
                f'      sha256 "{hashes[target]}"',
                "    end",
            ])
            if index == 0:
                formula.append("")
        formula.extend(["  end", ""])
    formula.extend([
        '  uses_from_macos "git"',
        "",
        "  def install",
        '    bin.install "contextcut"',
        '    doc.install "README.md"',
        "  end",
        "",
        "  test do",
        '    assert_match version.to_s, shell_output("#{bin}/contextcut --version")',
        '    (testpath/"example.py").write "answer = 42\\n"',
        '    assert_match "answer = 42", shell_output("#{bin}/contextcut #{testpath} --budget 2k")',
        "  end",
        "end",
        "",
    ])
    (out / "contextcut.rb").write_text("\n".join(formula))
    files = [out / filename(target) for target in TARGETS] + [out / "contextcut.rb"]
    (out / "SHA256SUMS").write_text("".join(f"{digest(path)}  {path.name}\n" for path in sorted(files)))
    print("Generated contextcut.rb and SHA256SUMS from all five release archives")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("package")
    build.add_argument("--target", choices=TARGETS, required=True)
    build.add_argument("--binary", type=Path, required=True)
    build.add_argument("--out", type=Path, default=Path("dist"))
    check = commands.add_parser("smoke")
    check.add_argument("archive", type=Path)
    checksums = commands.add_parser("manifest")
    checksums.add_argument("--out", type=Path, default=Path("dist"))
    commands.add_parser("version")
    args = parser.parse_args()
    if args.command == "package":
        package(args.binary, args.target, args.out)
    elif args.command == "smoke":
        smoke(args.archive.resolve())
    elif args.command == "manifest":
        manifest(args.out)
    else:
        print(version())


if __name__ == "__main__":
    main()
