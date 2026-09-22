import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

import yaml


ROOT = Path(__file__).resolve().parent


def load_yaml(path: Path):
    return yaml.safe_load(path.read_text())


class BundleTests(unittest.TestCase):
    def test_compose_never_publishes_painter_beyond_loopback(self):
        compose = load_yaml(ROOT / "compose.yaml")
        self.assertEqual(
            compose["services"]["painter"]["ports"],
            ["127.0.0.1:18080:8080"],
        )

    def test_twitch_key_is_a_file_secret_not_an_environment_value(self):
        compose = load_yaml(ROOT / "compose.yaml")
        compose_text = (ROOT / "compose.yaml").read_text()
        self.assertEqual(
            compose["secrets"]["twitch_stream_key"]["file"],
            "/etc/abbey-road-stream/twitch_stream_key",
        )
        self.assertIn("twitch_stream_key", compose["services"]["relay"]["secrets"])
        self.assertIn("/run/secrets/twitch_stream_key", compose_text)
        self.assertNotIn("TWITCH_STREAM_KEY=", compose_text)

    def test_relay_requires_healthy_painter_and_restarts(self):
        relay = load_yaml(ROOT / "compose.yaml")["services"]["relay"]
        self.assertEqual(
            relay["depends_on"]["painter"]["condition"],
            "service_healthy",
        )
        self.assertEqual(relay["restart"], "unless-stopped")

    def test_painter_has_private_healthcheck_and_restart_policy(self):
        painter = load_yaml(ROOT / "compose.yaml")["services"]["painter"]
        self.assertEqual(painter["restart"], "unless-stopped")
        self.assertIn("http://127.0.0.1:8080/health", " ".join(painter["healthcheck"]["test"]))

    def test_relay_has_progress_healthcheck(self):
        relay = load_yaml(ROOT / "compose.yaml")["services"]["relay"]
        self.assertEqual(relay["healthcheck"]["test"], ["CMD", "/usr/local/bin/healthcheck.sh"])
        self.assertIn("relay-progress", (ROOT / "relay" / "healthcheck.sh").read_text())

    def test_qualifier_consumes_private_hls_to_null_muxer(self):
        qualify = load_yaml(ROOT / "compose.yaml")["services"]["qualify"]
        self.assertEqual(qualify["profiles"], ["qualify"])
        self.assertEqual(qualify["environment"]["HLS_URL"], "http://painter:8080/api/stream")
        self.assertEqual(qualify["entrypoint"][-3:], ["-f", "null", "-"])

    def test_relay_script_drops_privileges_and_has_required_ffmpeg_contract(self):
        script = (ROOT / "relay" / "relay.sh").read_text()
        required_fragments = (
            "set +x",
            "/run/secrets/twitch_stream_key",
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
        for fragment in required_fragments:
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, script)

    def test_healthcheck_accepts_only_fresh_progress(self):
        healthcheck = ROOT / "relay" / "healthcheck.sh"
        progress = Path("/tmp/relay-progress")
        original = progress.read_bytes() if progress.exists() else None
        try:
            progress.unlink(missing_ok=True)
            self.assertNotEqual(subprocess.run([healthcheck], check=False).returncode, 0)
            progress.touch()
            self.assertEqual(subprocess.run([healthcheck], check=False).returncode, 0)
            old = time.time() - 46
            os.utime(progress, (old, old))
            self.assertNotEqual(subprocess.run([healthcheck], check=False).returncode, 0)
        finally:
            if original is None:
                progress.unlink(missing_ok=True)
            else:
                progress.write_bytes(original)

    def test_painter_example_contains_only_exact_non_secret_configuration(self):
        expected = """PORT=8080
PUBLIC_MODE=true
ALLOWED_ORIGIN=https://worthyrae.com
OUTPUT_WIDTH=1920
LIVE_STYLE=loaded_bristle
LOADED_PAINT_STRIDE=2
LOADED_CV_THREADS=4
RENDER_WORKERS=6
ENCODE_THREADS=2
ENCODE_CRF=30
ENCODE_GOP_SECONDS=2
NUMPY_MADVISE_HUGEPAGE=0
"""
        self.assertEqual((ROOT / "painter.env.example").read_text(), expected)

    def test_cloud_init_hardens_host_without_embedding_credentials_or_app_env(self):
        cloud_init = load_yaml(ROOT / "cloud-init.yaml")
        packages = set(cloud_init["packages"])
        self.assertTrue({"docker.io", "docker-compose-v2", "ufw", "unattended-upgrades"} <= packages)
        rendered = (ROOT / "cloud-init.yaml").read_text()
        required_fragments = (
            "install -d -m 0755 /opt/abbey-road-stream",
            "install -d -m 0700 /etc/abbey-road-stream",
            "systemctl enable --now ssh",
            "ufw default deny incoming",
            "ufw default allow outgoing",
            "ufw allow OpenSSH",
            "fallocate -l 1G /swapfile",
            "SystemMaxUse=250M",
            'Unattended-Upgrade::Automatic-Reboot "false";',
        )
        for fragment in required_fragments:
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, rendered)
        for forbidden in (
            "ssh_authorized_keys",
            "TWITCH_STREAM_KEY",
            "twitch_stream_key",
            "ALLOWED_ORIGIN",
            "RAILWAY_",
            "api_token",
            "git credential",
        ):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, rendered)


class VerifierTests(unittest.TestCase):
    def run_verifier(self, root: Path):
        return subprocess.run(
            [sys.executable, ROOT / "verify_bundle.py", "--root", root],
            check=False,
            capture_output=True,
            text=True,
        )

    def copy_bundle(self, destination: Path):
        shutil.copytree(ROOT, destination, ignore=shutil.ignore_patterns("__pycache__"))

    def assert_rejected(self, root: Path, reason: str):
        result = self.run_verifier(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(reason, result.stdout + result.stderr)

    def test_canonical_bundle_passes_static_verification(self):
        result = self.run_verifier(ROOT)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("bundle verification passed", result.stdout)

    def test_verifier_rejects_public_wildcard_binding(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            bundle = Path(temp_dir) / "bundle"
            self.copy_bundle(bundle)
            compose_path = bundle / "compose.yaml"
            compose_path.write_text(compose_path.read_text().replace("127.0.0.1:18080", "0.0.0.0:18080", 1))
            self.assert_rejected(bundle, "0.0.0.0")

    def test_verifier_rejects_environment_secret_placeholder(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            bundle = Path(temp_dir) / "bundle"
            self.copy_bundle(bundle)
            compose_path = bundle / "compose.yaml"
            compose_path.write_text(compose_path.read_text().replace(
                "TWITCH_INGEST_URL: rtmps://live.twitch.tv/app",
                "TWITCH_INGEST_URL: rtmps://live.twitch.tv/app\n      TWITCH_STREAM_KEY: ${TWITCH_STREAM_KEY}",
            ))
            self.assert_rejected(bundle, "${TWITCH_STREAM_KEY}")

    def test_verifier_rejects_world_readable_secret_source(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            bundle = Path(temp_dir) / "bundle"
            self.copy_bundle(bundle)
            secret_path = bundle / "test-secret"
            secret_path.write_text("not-a-real-secret\n")
            secret_path.chmod(0o644)
            compose_path = bundle / "compose.yaml"
            compose = load_yaml(compose_path)
            compose["secrets"]["twitch_stream_key"]["file"] = str(secret_path)
            compose_path.write_text(yaml.safe_dump(compose, sort_keys=False))
            self.assert_rejected(bundle, "mode 0600")

    def test_verifier_rejects_missing_restart_policies(self):
        for service in ("painter", "relay"):
            with self.subTest(service=service), tempfile.TemporaryDirectory() as temp_dir:
                bundle = Path(temp_dir) / "bundle"
                self.copy_bundle(bundle)
                compose_path = bundle / "compose.yaml"
                compose = load_yaml(compose_path)
                del compose["services"][service]["restart"]
                compose_path.write_text(yaml.safe_dump(compose, sort_keys=False))
                self.assert_rejected(bundle, f"{service} restart")

    def test_verifier_rejects_relay_healthcheck_without_progress_file(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            bundle = Path(temp_dir) / "bundle"
            self.copy_bundle(bundle)
            healthcheck_path = bundle / "relay" / "healthcheck.sh"
            healthcheck_path.chmod(0o755)
            healthcheck_path.write_text(healthcheck_path.read_text().replace("relay-progress", "other-progress"))
            self.assert_rejected(bundle, "relay-progress")


if __name__ == "__main__":
    unittest.main()
