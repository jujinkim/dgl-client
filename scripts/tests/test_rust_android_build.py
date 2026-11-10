from pathlib import Path
import os
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import build_rust_android as build


class AndroidRustBuildTest(unittest.TestCase):
    def test_all_abis_and_binding_generation_on_each_host(self):
        for system, host, library, binary, cc_suffix, ar_suffix in (
            ("Windows", "windows-x86_64", "dgl_uniffi.dll", "uniffi-bindgen.exe", ".cmd", ".exe"),
            ("Darwin", "darwin-x86_64", "libdgl_uniffi.dylib", "uniffi-bindgen", "", ""),
            ("Linux", "linux-x86_64", "libdgl_uniffi.so", "uniffi-bindgen", "", ""),
        ):
            with self.subTest(system=system), tempfile.TemporaryDirectory(prefix="android build ") as directory:
                root = Path(directory)
                sdk = root / "SDK with spaces"
                tools = sdk / "ndk" / build.NDK_VERSION / "toolchains/llvm/prebuilt" / host / "bin"
                tools.mkdir(parents=True)
                commands = []

                def run(args, **kwargs):
                    commands.append((args, kwargs.get("env", {})))
                    if args[0].endswith(binary):
                        generated = Path(args[args.index("--out-dir") + 1])
                        generated.mkdir(parents=True)
                        (generated / "dgl_core.kt").write_text("package uniffi.dgl_core\n")
                    elif "--target" in args:
                        target = args[args.index("--target") + 1]
                        output = root / "target/android" / target / "release/libdgl_uniffi.so"
                        output.parent.mkdir(parents=True)
                        output.write_bytes(target.encode())

                with patch.object(build, "ROOT", root), patch.object(build.platform, "system", return_value=system), \
                     patch.dict(os.environ, {"ANDROID_HOME": str(sdk)}, clear=True), patch.object(build, "run", side_effect=run):
                    build.main()

                self.assertEqual(commands[0][1]["CARGO_TARGET_DIR"], str(root / "target"))
                bindgen = commands[1][0]
                self.assertEqual(bindgen[0], str(root / "target/debug" / binary))
                self.assertEqual(bindgen[bindgen.index("--library") + 1], str(root / "target/debug" / library))
                self.assertTrue((root / "coreBindings/src/main/kotlin/uniffi/dgl_core/dgl_core.kt").is_file())
                self.assertEqual(len(commands), 6)
                for (abi, (target, clang)), (args, env) in zip(build.TARGETS.items(), commands[2:]):
                    self.assertIn(target, args)
                    self.assertEqual(env[f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"], str(tools / f"{clang}34-clang{cc_suffix}"))
                    self.assertEqual(env[f"AR_{target.replace('-', '_')}"], str(tools / f"llvm-ar{ar_suffix}"))
                    self.assertEqual((root / "coreBindings/src/main/jniLibs" / abi / "libdgl_uniffi.so").read_bytes(), target.encode())

    def test_macos_cross_build_preserves_explicit_environment(self):
        with patch.object(build.platform, "system", return_value="Darwin"), patch.object(build.subprocess, "run") as run:
            build.run(["cargo", "build"], env={"CC": "ndk-clang"})
        self.assertEqual(run.call_args.kwargs["env"]["CC"], "ndk-clang")
        self.assertIn("DEVELOPER_DIR", run.call_args.kwargs["env"])

    def test_unsupported_host_is_rejected(self):
        with patch.object(build.platform, "system", return_value="Unknown"), self.assertRaises(SystemExit):
            build.host_tools()


if __name__ == "__main__":
    unittest.main()
