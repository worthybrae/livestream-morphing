#!/usr/bin/env python3
"""Static safety checks for the Hetzner painter/relay deployment bundle."""

import argparse
from pathlib import Path
import stat
import sys

import yaml


EXPECTED_SECRET_SOURCE = "/etc/abbey-road-stream/twitch_stream_key"
EXPECTED_SECRET_TARGET = "/run/secrets/twitch_stream_key"
EXPECTED_PAINTER_PORT = "127.0.0.1:18080:8080"


def load_yaml(path: Path, errors: list[str]):
    try:
        return yaml.safe_load(path.read_text())
    except (OSError, yaml.YAMLError) as error:
        errors.append(f"cannot load {path.name}: {error}")
        return {}


def require(condition: bool, message: str, errors: list[str]):
    if not condition:
        errors.append(message)


def verify(root: Path) -> list[str]:
    errors: list[str] = []
    compose_path = root / "compose.yaml"
    compose = load_yaml(compose_path, errors)
    try:
        compose_text = compose_path.read_text()
    except OSError:
        compose_text = ""

    require("0.0.0.0" not in compose_text, "compose.yaml must reject 0.0.0.0 bindings", errors)
    require(
        "${TWITCH_STREAM_KEY}" not in compose_text,
        "compose.yaml must reject ${TWITCH_STREAM_KEY}; use the file secret",
        errors,
    )
    require(
        "TWITCH_STREAM_KEY" not in compose_text,
        "compose.yaml must not define TWITCH_STREAM_KEY in the environment",
        errors,
    )

    services = compose.get("services", {}) if isinstance(compose, dict) else {}
    painter = services.get("painter", {})
    relay = services.get("relay", {})
    qualify = services.get("qualify", {})

    require(
        painter.get("ports") == [EXPECTED_PAINTER_PORT],
        f"painter ports must be exactly [{EXPECTED_PAINTER_PORT!r}]",
        errors,
    )
    for service_name, service in (("painter", painter), ("relay", relay)):
        require(
            service.get("restart") == "unless-stopped",
            f"{service_name} restart policy must be unless-stopped",
            errors,
        )

    painter_health = " ".join(painter.get("healthcheck", {}).get("test", []))
    require(
        "http://127.0.0.1:8080/health" in painter_health,
        "painter health check must use its loopback /health endpoint",
        errors,
    )
    require(
        relay.get("depends_on", {}).get("painter", {}).get("condition") == "service_healthy",
        "relay must wait for a healthy painter",
        errors,
    )
    require(
        relay.get("healthcheck", {}).get("test") == ["CMD", "/usr/local/bin/healthcheck.sh"],
        "relay health check must execute /usr/local/bin/healthcheck.sh",
        errors,
    )

    relay_environment = relay.get("environment", {})
    require(
        relay_environment == {
            "HLS_URL": "http://painter:8080/api/stream",
            "TWITCH_INGEST_URL": "rtmps://live.twitch.tv/app",
        },
        "relay environment must contain only non-secret HLS_URL and TWITCH_INGEST_URL",
        errors,
    )
    require(
        relay.get("secrets") == ["twitch_stream_key"],
        "relay must mount only the twitch_stream_key file secret",
        errors,
    )

    secret_config = compose.get("secrets", {}).get("twitch_stream_key", {})
    secret_source = secret_config.get("file")
    require(
        secret_source == EXPECTED_SECRET_SOURCE,
        f"Twitch key source must be {EXPECTED_SECRET_SOURCE}",
        errors,
    )
    if secret_source and Path(secret_source).exists():
        secret_stat = Path(secret_source).stat()
        mode = stat.S_IMODE(secret_stat.st_mode)
        require(mode == 0o600, f"secret source must have mode 0600, found {mode:04o}", errors)
        require(secret_stat.st_uid == 0, "secret source must be root-owned", errors)

    require(qualify.get("profiles") == ["qualify"], "qualification service must use the qualify profile", errors)
    qualify_command = qualify.get("entrypoint", [])
    require(
        qualify_command[-3:] == ["-f", "null", "-"],
        "qualification service must consume HLS into FFmpeg's null muxer",
        errors,
    )

    relay_script_path = root / "relay" / "relay.sh"
    healthcheck_path = root / "relay" / "healthcheck.sh"
    dockerfile_path = root / "relay" / "Dockerfile"
    try:
        relay_script = relay_script_path.read_text()
    except OSError as error:
        errors.append(f"cannot load relay.sh: {error}")
        relay_script = ""
    try:
        healthcheck = healthcheck_path.read_text()
    except OSError as error:
        errors.append(f"cannot load healthcheck.sh: {error}")
        healthcheck = ""
    try:
        dockerfile = dockerfile_path.read_text()
    except OSError as error:
        errors.append(f"cannot load relay Dockerfile: {error}")
        dockerfile = ""

    require("/tmp/relay-progress" in healthcheck, "relay health check must inspect relay-progress", errors)
    require("-lt 45" in healthcheck, "relay health check must reject progress aged 45 seconds", errors)
    require("set +x" in relay_script, "relay must disable shell tracing before reading the secret", errors)
    require(EXPECTED_SECRET_TARGET in relay_script, f"relay must read {EXPECTED_SECRET_TARGET}", errors)
    require(
        relay_script.find("set +x") < relay_script.find(EXPECTED_SECRET_TARGET),
        "relay must disable shell tracing before reading the secret",
        errors,
    )
    required_relay_fragments = (
        "rm -f /tmp/relay-progress",
        "exec setpriv --reuid=65534 --regid=65534 --clear-groups ffmpeg",
        "-loglevel quiet",
        "-reconnect 1",
        "-reconnect_streamed 1",
        "-reconnect_delay_max 5",
        "-progress /tmp/relay-progress",
        "-map 0:v:0",
        "anullsrc=channel_layout=stereo:sample_rate=48000",
        "-c:v copy",
        "-c:a aac",
        "-b:a 64k",
        "-ar 48000",
        "-f flv",
        '"${TWITCH_INGEST_URL}/${stream_key}"',
    )
    for fragment in required_relay_fragments:
        require(fragment in relay_script, f"relay.sh is missing required contract: {fragment}", errors)

    require("FROM debian:bookworm-slim" in dockerfile, "relay image must use Debian Bookworm slim", errors)
    for package in ("ffmpeg", "ca-certificates", "util-linux"):
        require(package in dockerfile, f"relay image must install {package}", errors)
    require("USER " not in dockerfile, "relay entrypoint must start as root to read the 0600 secret", errors)

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    errors = verify(args.root.resolve())
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print("bundle verification passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
