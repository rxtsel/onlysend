"""Helpers for the manually dispatched release; never read signing private keys."""
import argparse
import base64
import os
import tempfile
import datetime
import json
import re
import shutil
import subprocess
import tomllib
from pathlib import Path
from urllib.parse import quote

PLATFORMS = {
    "linux-x86_64": ".AppImage",
    "windows-x86_64": ".exe",
    "darwin-aarch64": ".app.tar.gz",
    "darwin-x86_64": ".app.tar.gz",
}


def semver(value):
    if not re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", value):
        raise ValueError("Use a stable SemVer such as 0.2.0 (no v prefix)")
    components = tuple(map(int, value.split(".")))
    if any(component > 2**64 - 1 for component in components):
        raise ValueError("Version component exceeds SemVer limits")
    return components


def assert_newest(version, tags):
    candidate = semver(version)
    for tag in tags:
        if re.fullmatch(r"v\d+\.\d+\.\d+", tag) and semver(tag[1:]) > candidate:
            raise ValueError("A newer release tag exists; refusing to publish an older version")


def replace_version_block(text, heading, name, version):
    blocks = list(re.finditer(r"(?ms)^" + re.escape(heading) + r"[^\n]*\n.*?(?=^\[|\Z)", text))
    matches = [block for block in blocks if name is None or re.search(r'^name\s*=\s*"' + re.escape(name) + r'"\s*$', block.group(), re.M)]
    if len(matches) != 1:
        raise ValueError("Expected exactly one application version block")
    block = matches[0]
    updated, count = re.subn(r'(?m)^(version\s*=\s*)"[^"]+"', lambda match: match[1] + '"' + version + '"', block.group())
    if count != 1:
        raise ValueError("Expected exactly one application version")
    return text[:block.start()] + updated + text[block.end():]


def prepare(root, version, tags):
    new = semver(version)
    cargo_path = root / "src-tauri/Cargo.toml"
    cargo = tomllib.loads(cargo_path.read_text())
    package_path = root / "package.json"
    config_path = root / "src-tauri/tauri.conf.json"
    package = json.loads(package_path.read_text())
    config = json.loads(config_path.read_text())
    current = cargo["package"]["version"]
    if package["version"] != current or config["version"] != current:
        raise ValueError("Application version files disagree")
    if new <= semver(current):
        raise ValueError("Release version must exceed the current application version")
    for tag in tags:
        if re.fullmatch(r"v\d+\.\d+\.\d+", tag) and new <= semver(tag[1:]):
            raise ValueError("Release version must exceed all existing stable release tags")
    lock_path = root / "src-tauri/Cargo.lock"
    lock = lock_path.read_text()
    own = [entry for entry in tomllib.loads(lock)["package"] if entry["name"] == cargo["package"]["name"]]
    if len(own) != 1 or own[0]["version"] != current:
        raise ValueError("Cargo.lock application version disagrees")
    # Validate every replacement before writing any version file.
    updated_cargo = replace_version_block(cargo_path.read_text(), "[package]", None, version)
    updated_lock = replace_version_block(lock, "[[package]]", cargo["package"]["name"], version)
    package["version"] = config["version"] = version
    cargo_path.write_text(updated_cargo)
    lock_path.write_text(updated_lock)
    package_path.write_text(json.dumps(package, indent=2, ensure_ascii=False) + "\n")
    config_path.write_text(json.dumps(config, indent=2, ensure_ascii=False) + "\n")


def stage(source, output, platform):
    """Validate one platform's bundles before uploading a workflow artifact."""
    if platform not in PLATFORMS:
        raise ValueError("Unsupported release platform")
    if not source.is_dir():
        raise ValueError(f"Bundle directory does not exist: {source}")
    extension = PLATFORMS[platform]
    required = [extension]
    if platform == "linux-x86_64":
        required.append(".deb")
    elif platform.startswith("darwin-"):
        required.append(".dmg")
    files = []
    for suffix in required:
        matches = [file for file in source.rglob("*") if file.is_file() and not file.is_symlink() and file.name.endswith(suffix)]
        if len(matches) != 1 or matches[0].stat().st_size == 0:
            found = sorted(str(file.relative_to(source)) for file in source.rglob("*") if file.is_file() and not file.is_symlink())
            hint = " Build macOS with --bundles app,dmg to generate the updater archive." if platform.startswith("darwin-") and suffix == ".app.tar.gz" else ""
            raise ValueError(f"Expected exactly one nonempty {suffix} bundle for {platform}; matched {len(matches)}. Files found: {found}.{hint}")
        files.append(matches[0])
        if suffix == extension:
            signature = Path(str(matches[0]) + ".sig")
            if not signature.is_file() or signature.is_symlink() or not signature.read_text().strip():
                raise ValueError(f"Missing updater signature for {platform}")
            files.append(signature)
    output.mkdir(parents=True, exist_ok=False)
    for file in files:
        shutil.copyfile(file, output / file.name)


def collect(source, output, version, repository, notes):
    semver(version)
    if not re.fullmatch(r"[\w.-]+/[\w.-]+", repository):
        raise ValueError("Invalid GitHub repository")
    output.mkdir(parents=True, exist_ok=False)
    suffixes = (".deb", ".AppImage", ".exe", ".dmg", ".app.tar.gz", ".sig")
    for file in source.rglob("*"):
        if not file.is_file() or file.is_symlink() or not file.name.endswith(suffixes):
            continue
        artifact = file.relative_to(source).parts[0]
        platform = next((key for key in PLATFORMS if artifact.startswith(key + "-")), None)
        if platform is None:
            continue
        destination = output / (platform + "-" + file.name)
        if destination.exists():
            if destination.read_bytes() != file.read_bytes():
                raise ValueError("Conflicting artifact filenames")
        else:
            shutil.copyfile(file, destination)
    platforms = {}
    for platform, extension in PLATFORMS.items():
        assets = [file for file in output.iterdir() if file.name.startswith(platform + "-") and file.name.endswith(extension)]
        if len(assets) != 1:
            raise ValueError(f"Expected one updater artifact for {platform}")
        asset = assets[0]
        signature_file = Path(str(asset) + ".sig")
        signature = signature_file.read_text().strip()
        if not signature:
            raise ValueError(f"Missing signature for {platform}")
        platforms[platform] = {
            "url": f"https://github.com/{repository}/releases/download/v{version}/{quote(asset.name)}",
            "signature": signature,
        }
    for platform, extension in [("linux-x86_64", ".deb"), ("darwin-aarch64", ".dmg"), ("darwin-x86_64", ".dmg")]:
        if not any(file.name.startswith(platform + "-") and file.name.endswith(extension) for file in output.iterdir()):
            raise ValueError(f"Missing {extension} installer for {platform}")
    manifest = {
        "version": version,
        "notes": notes,
        "pub_date": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "platforms": platforms,
    }
    (output / "latest.json").write_text(json.dumps(manifest, indent=2) + "\n")


def verify(output, public_key):
    # Tauri wraps the standard minisign public key and signature text in base64.
    # Use the real verifier, not a custom cryptographic implementation.
    with tempfile.TemporaryDirectory() as temp:
        key = Path(temp) / "public.pub"
        key.write_bytes(base64.b64decode(public_key.strip(), validate=True))
        signatures = list(output.glob("*.sig"))
        if not signatures:
            raise ValueError("No signed artifacts to verify")
        for index, encoded in enumerate(signatures):
            signature = Path(temp) / f"signature-{index}.sig"
            signature.write_bytes(base64.b64decode(encoded.read_text().strip(), validate=True))
            asset = Path(str(encoded)[:-4])
            subprocess.run(["minisign", "-V", "-q", "-p", str(key), "-x", str(signature), "-m", str(asset)], check=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare_args = commands.add_parser("prepare")
    prepare_args.add_argument("version")
    collect_args = commands.add_parser("collect")
    for name in ("source", "output", "version", "repository", "notes"):
        collect_args.add_argument(name)
    stage_args = commands.add_parser("stage")
    for name in ("source", "output", "platform"):
        stage_args.add_argument(name)
    verify_args = commands.add_parser("verify")
    verify_args.add_argument("output")
    args = parser.parse_args()
    if args.command == "prepare":
        tags = subprocess.check_output(["git", "tag", "--list"], text=True).splitlines()
        prepare(Path.cwd(), args.version, tags)
    elif args.command == "stage":
        stage(Path(args.source), Path(args.output), args.platform)
    elif args.command == "verify":
        verify(Path(args.output), os.environ["PUBLIC_KEY"])
    else:
        collect(Path(args.source), Path(args.output), args.version, args.repository, Path(args.notes).read_text())
