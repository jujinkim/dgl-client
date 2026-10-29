import io
import sys
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from collect_rust_android import ABIS, verify_rust_payload


class RustApkTest(unittest.TestCase):
    def archive(self, abis=ABIS, extra_dex=b"", rust_dex=True, foreign_abi=False):
        data = io.BytesIO()
        with zipfile.ZipFile(data, "w") as apk:
            for abi in abis:
                apk.writestr(f"lib/{abi}/libdgl_uniffi.so", b"native fixture")
            if foreign_abi:
                apk.writestr("lib/mips/libjnidispatch.so", b"unsupported fixture")
            apk.writestr("assets/third-party-rust/DEPENDENCIES.json", "[]")
            apk.writestr("classes.dex", b"Lkim/jujin/dglc/RustClientConnection;\0"
                         b"Lkim/jujin/dglc/RustKeyTools;\0Luniffi/dgl_core/NativeConnection;"
                         if rust_dex else b"")
            apk.writestr("classes2.dex", extra_dex)
        return zipfile.ZipFile(data)

    def test_rust_app_has_four_library_hashes(self):
        with self.archive() as apk:
            self.assertEqual(set(verify_rust_payload(apk)), ABIS)

    def test_missing_abi_rejected(self):
        with self.archive(abis={"arm64-v8a"}) as apk, self.assertRaisesRegex(ValueError, "four Rust ABIs"):
            verify_rust_payload(apk)

    def test_foreign_abi_without_rust_library_rejected(self):
        with self.archive(foreign_abi=True) as apk, self.assertRaisesRegex(ValueError, "four Rust ABIs"):
            verify_rust_payload(apk)

    def test_test_only_engine_and_dependencies_rejected_in_secondary_dex(self):
        for descriptor in (b"Lkim/jujin/dglc/ClientModel;", b"Lkim/jujin/dglc/ssh/SshjTransport;",
                           b"Lkim/jujin/dglc/core/ControlParser;", b"Lkim/jujin/dglc/core/TmuxWire;",
                           b"Lkim/jujin/rovessh/ClientModel;", b"Lkim/jujin/rovessh/MainActivity;",
                           b"Lnet/schmizz/sshj/SSHClient;",
                           b"Lorg/bouncycastle/crypto/CipherParameters;"):
            with self.subTest(descriptor=descriptor), self.archive(extra_dex=descriptor) as apk:
                with self.assertRaisesRegex(ValueError, "Retired engine/dependency"):
                    verify_rust_payload(apk)

    def test_unused_native_libraries_do_not_prove_rust_execution(self):
        with self.archive(rust_dex=False) as apk, self.assertRaisesRegex(ValueError, "Rust implementation missing"):
            verify_rust_payload(apk)
