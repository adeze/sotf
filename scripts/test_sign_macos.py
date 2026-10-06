"""Check desktop signing commands without certificates or Keychain access."""

import json
import os
from pathlib import Path
import plistlib
import re
import shlex
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/sign-macos.sh"
ENTITLEMENTS = ROOT / "builds/macos/entitlements.plist"


class SigningCommandsTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="sotf-signing-test-")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        tools = self.base / "tools"
        tools.mkdir()
        self.command_log = self.base / "codesign.jsonl"
        self.env = os.environ.copy()
        self.env.update(
            PATH=str(tools) + os.pathsep + self.env.get("PATH", ""),
            DEVELOPER_ID="QA_SIGNER_PLACEHOLDER",
            SOTF_SIGN_TEST_LOG=str(self.command_log),
        )
        self.write_tool(
            tools / "codesign",
            "import json, os, sys\n"
            "args = sys.argv[1:]\n"
            "with open(os.environ['SOTF_SIGN_TEST_LOG'], 'a') as log:\n"
            "    log.write(json.dumps(args) + '\\n')\n"
            "if '--verify' in args and args[-1] == os.environ.get('SOTF_SIGN_TEST_REJECT'):\n"
            "    sys.exit(42)\n",
        )
        self.write_tool(
            tools / "file",
            "import pathlib, sys\n"
            "path = pathlib.Path(sys.argv[-1])\n"
            "kind = 'Mach-O 64-bit executable arm64' if path.read_bytes().startswith(b'MACHO_QA') else 'data'\n"
            "print(str(path) + ': ' + kind)\n",
        )

    @staticmethod
    def write_tool(path, body):
        path.write_text("#!" + sys.executable + "\n" + body)
        path.chmod(0o755)

    def macho(self, relative_path):
        path = self.base / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"MACHO_QA")
        return path

    def invoke(self, function, target):
        self.assertRegex(function, r"^[a-z_]+$")
        return subprocess.run(
            [
                "/bin/bash", "-c",
                'script_path=$1; target_path=$2; set --; source "$script_path"; '
                + function + ' "$target_path"',
                "signing-test", str(SCRIPT), str(target),
            ],
            env=self.env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def calls(self):
        return [json.loads(line) for line in self.command_log.read_text().splitlines()]

    def signed_call(self, calls, target):
        matches = [args for args in calls if "--sign" in args and args[-1] == str(target)]
        self.assertEqual(len(matches), 1)
        self.assertIn("runtime", matches[0])
        self.assertIn("--timestamp", matches[0])
        return matches[0]

    def assert_desktop_entitlements(self, args):
        self.assertIn("--entitlements", args)
        self.assertEqual(args[args.index("--entitlements") + 1], str(ENTITLEMENTS))

    def test_desktop_payload_keeps_entitlements_on_final_bundle_signature(self):
        executable = self.macho("payload/Renamed Desktop.app/Contents/MacOS/sotf-desktop")
        library = self.macho("payload/Renamed Desktop.app/Contents/Frameworks/libfixture.dylib")
        tui = self.macho("payload/sotf-tui")
        bundle = executable.parents[2]
        result = self.invoke("sign_payload_code", self.base / "payload")
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        leaf_call = self.signed_call(calls, executable)
        library_call = self.signed_call(calls, library)
        bundle_call = self.signed_call(calls, bundle)
        self.assert_desktop_entitlements(leaf_call)
        self.assert_desktop_entitlements(bundle_call)
        self.assertNotIn("--deep", bundle_call)
        self.assertNotIn("--entitlements", library_call)
        self.assertNotIn("--entitlements", self.signed_call(calls, tui))
        self.assertLess(calls.index(leaf_call), calls.index(bundle_call))
        self.assertLess(calls.index(library_call), calls.index(bundle_call))
        self.assertTrue(any("--verify" in args and "--strict" in args and args[-1] == str(bundle) for args in calls))

    def test_versioned_desktop_binary_gets_entitlements(self):
        executable = self.macho("sotf-desktop-0.7.14-macos-arm64")
        result = self.invoke("sign_file", executable)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_desktop_entitlements(self.signed_call(self.calls(), executable))

    def test_other_bundle_preserves_existing_signing_behavior(self):
        executable = self.macho("Other.app/Contents/MacOS/other")
        bundle = executable.parents[2]
        result = self.invoke("sign_code_bundle", bundle)
        self.assertEqual(result.returncode, 0, result.stderr)
        args = self.signed_call(self.calls(), bundle)
        self.assertIn("--deep", args)
        self.assertNotIn("--entitlements", args)

    def test_failed_bundle_verification_stops_signing(self):
        executable = self.macho("Desktop.app/Contents/MacOS/sotf-desktop")
        bundle = executable.parents[2]
        self.env["SOTF_SIGN_TEST_REJECT"] = str(bundle)
        result = self.invoke("sign_code_bundle", bundle)
        self.assertEqual(result.returncode, 42)

    def test_canonical_policy_allows_audio_input(self):
        policy = plistlib.loads(ENTITLEMENTS.read_bytes())
        self.assertIs(policy.get("com.apple.security.device.audio-input"), True)

    def test_release_recipes_reference_existing_canonical_policy(self):
        justfile = (ROOT / "Justfile").read_text()
        for recipe in ("run-gpui-release", "run-gpui-leaks"):
            with self.subTest(recipe=recipe):
                match = re.search(r"^" + recipe + r":\n((?:\t[^\n]*\n)+)", justfile, re.MULTILINE)
                self.assertIsNotNone(match)
                commands = [shlex.split(line.strip()) for line in match[1].splitlines() if "codesign " in line]
                self.assertEqual(len(commands), 1)
                args = commands[0]
                path = ROOT / args[args.index("--entitlements") + 1]
                self.assertEqual(path, ENTITLEMENTS)
                self.assertTrue(path.is_file())


if __name__ == "__main__":
    unittest.main()
