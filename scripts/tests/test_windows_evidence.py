import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collect_artifacts as collector
from evidence import ROOT, safe_relative, sha256, write_report
from verify_android_results import EXPECTED_ANDROID, verify as verify_android
from verify_test_results import EXPECTED, verify as verify_jvm
from verify_windows_installers import installers


class CheckoutTests(unittest.TestCase):
    def test_original_bytes_survive_commit_and_checkout_both_autocrlf_modes(self):
        paths = [ROOT / 'KICKOFF_PROMPT.md'] + collector.license_files()
        originals = {p.relative_to(ROOT).as_posix(): p.read_bytes() for p in paths}
        for autocrlf in ('true', 'false'):
            with self.subTest(autocrlf=autocrlf), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                shutil.copyfile(ROOT / '.gitattributes', root / '.gitattributes')
                for name, content in originals.items():
                    p = root / name
                    p.parent.mkdir(parents=True, exist_ok=True)
                    p.write_bytes(content)
                (root / 'sample.py').write_bytes(b'print("source")\r\n')
                (root / 'sample.bat').write_bytes(b'@echo off\n')
                def git(*args):
                    return subprocess.check_output(['git', '-c', 'core.autocrlf=' + autocrlf,
                        '-c', 'core.safecrlf=false', '-c', 'core.attributesFile=' + os.devnull,
                        '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=' + str(root / 'no-hooks'),
                        '-c', 'user.name=Checkout Test', '-c', 'user.email=test@example.invalid', *args],
                        cwd=root, stderr=subprocess.PIPE)
                git('init', '-q')
                git('add', '.')
                git('commit', '-qm', 'byte preservation fixture')
                # Removing the files forces a real smudge/checkout, not an up-to-date no-op.
                for name in [*originals, 'sample.py', 'sample.bat']:
                    (root / name).unlink()
                git('checkout', 'HEAD', '--', '.')
                for name, content in originals.items():
                    self.assertEqual(content, (root / name).read_bytes(), name)
                    if name.startswith('licenses/evidence/'):
                        self.assertEqual(Path(name).stem, sha256(root / name))
                self.assertEqual(b'print("source")\n', (root / 'sample.py').read_bytes())
                self.assertEqual(b'@echo off\r\n', (root / 'sample.bat').read_bytes())


class ReportTests(unittest.TestCase):
    def test_unsafe_windows_and_unix_paths_rejected(self):
        for name in ('C:/file', 'C:\\file', '\\server\\file', '../file', '/file', 'a/../b', 'a//b', 'a\\b'):
            with self.subTest(name=name), self.assertRaises(ValueError):
                safe_relative(name)
        self.assertEqual('고지/라이선스.txt', safe_relative('고지/라이선스.txt'))

    def test_missing_empty_and_ambiguous_installers_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for kind in ('msi', 'exe'):
                d = root / 'app/build/compose/binaries/main' / kind
                d.mkdir(parents=True)
                (d / ('app.' + kind)).write_bytes(b'fixture')
            self.assertEqual({'msi', 'exe'}, set(installers(root)))
            p = root / 'app/build/compose/binaries/main/msi/app.msi'
            p.write_bytes(b'')
            with self.assertRaises(ValueError): installers(root)
            p.unlink()
            with self.assertRaises(ValueError): installers(root)
            p.write_bytes(b'fixture')
            shutil.copyfile(p, p.with_name('another.msi'))
            with self.assertRaises(ValueError): installers(root)

    def test_required_jvm_tests_and_skips(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            suite = ET.Element('testsuite')
            for name in sorted(EXPECTED):
                cls, method = name.rsplit('.', 1)
                case = ET.SubElement(suite, 'testcase', classname=cls, name=method + '[desktop]')
                if any('.' + c + '.' in name for c in ('IntegrationTest', 'RecoveryTest', 'PerformanceTest')):
                    ET.SubElement(case, 'skipped')
            path = root / 'TEST-fixture.xml'
            ET.ElementTree(suite).write(path)
            self.assertEqual(14, verify_jvm(root, 'windows')['passed'])
            dpapi = next(c for c in suite if 'WindowsSecurityTest' in c.attrib['classname'])
            ET.SubElement(dpapi, 'skipped')
            ET.ElementTree(suite).write(path)
            with self.assertRaises(ValueError): verify_jvm(root, 'windows')
            suite.remove(dpapi)
            ET.ElementTree(suite).write(path)
            with self.assertRaises(ValueError): verify_jvm(root, 'windows')

    def test_android_missing_and_skipped_cases_rejected(self):
        output = ''
        for name in sorted(EXPECTED_ANDROID):
            cls, method = name.rsplit('.', 1)
            output += f'INSTRUMENTATION_STATUS: class={cls}\nINSTRUMENTATION_STATUS: test={method}\nINSTRUMENTATION_STATUS_CODE: 0\n'
        output += 'OK (4 tests)\n'
        self.assertEqual(4, verify_android(output)['passed'])
        for broken in (output.replace('CODE: 0', 'CODE: -3', 1), 'OK (4 tests)', output.replace('OK (4 tests)', 'OK (3 tests)')):
            with self.assertRaises(ValueError): verify_android(broken)

    def test_windows_collection_rejects_incomplete_failed_stale_or_modified_evidence(self):
        ctx = {'commit': 'a' * 40, 'runId': '1', 'runAttempt': '1', 'os': 'Windows'}
        base = {'schema': 1, 'complete': True, 'status': 'passed', 'context': ctx}
        cases = {n: 'skipped' if any('.' + c + '.' in n for c in
                    ('IntegrationTest', 'RecoveryTest', 'PerformanceTest')) else 'passed' for n in EXPECTED}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sources = {k: root / ('installer.' + k) for k in ('msi', 'exe')}
            records = []
            for kind, path in sources.items():
                path.write_bytes(kind.encode())
                records.append({'kind': kind, 'name': path.name, 'sha256': sha256(path), 'status': 'passed',
                    'unregistered': True, 'noResidualFiles': True, 'noticeFiles': 477, 'installExitCode': 0,
                    'registration': {'productCode': '{fixture}'},
                    'launch': {'status': 'passed', 'aliveSeconds': 8, 'scope': 'process-liveness-only'},
                    'replacementLaunch': {'status': 'passed', 'aliveSeconds': 8, 'scope': 'process-liveness-only'},
                    'jediterm': [{'name': name + '-3.59.jar', 'originalSha256': 'b'*64, 'rebuiltSha256': 'c'*64}
                                 for name in ('jediterm-core', 'jediterm-ui')],
                    'temurin': {'vendor': 'Eclipse Adoptium', 'runtimeLegalFiles': 1, 'sourceSha256': 'd'*64, 'jvmSha256': 'e'*64}})
            reports = {'windows-tests': dict(base, mode='windows', cases=cases),
                'windows-dpapi': dict(base, checks={'ownRoundTrips': 2, 'foreignReadable': 2, 'foreignDecryptDenied': 2, 'cleanup': True}),
                'windows-installation': dict(base, installers=records)}
            def save(data):
                for name, report in data.items(): write_report(root / (name + '.json'), report)
            save(reports)
            collector.verify_windows_reports(root, sources, ctx)
            mutations = [lambda r: r['windows-dpapi'].update(complete=False),
                lambda r: r['windows-installation'].update(status='failed'),
                lambda r: r['windows-tests']['context'].update(commit='b'*40),
                lambda r: r['windows-tests']['cases'].pop(next(iter(cases))),
                lambda r: r['windows-dpapi']['checks'].update(foreignReadable=0),
                lambda r: r['windows-installation']['installers'].pop(),
                lambda r: r['windows-installation']['installers'][0].update(noResidualFiles=False),
                lambda r: r['windows-installation']['installers'][0]['jediterm'].pop()]
            for mutate in mutations:
                broken = copy.deepcopy(reports); mutate(broken); save(broken)
                with self.assertRaises(ValueError): collector.verify_windows_reports(root, sources, ctx)
            save(reports)
            sources['msi'].write_bytes(b'tampered')
            with self.assertRaises(ValueError): collector.verify_windows_reports(root, sources, ctx)


class NoticeTests(unittest.TestCase):
    def test_notice_utf8_manifest_and_tampering(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bundle = root / 'bundle'
            files = ['THIRD_PARTY_NOTICES.md', 'licenses/dependencies.json', 'licenses/native-components.json',
                     'licenses/jediterm-rebuild/README.md', 'licenses/jediterm-rebuild/build.gradle.kts',
                     'jvm/release', 'jvm/legal/java.base/LICENSE', '고지/읽어주세요.txt']
            for name in files:
                p = bundle / name; p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text('라이선스 원문\n', encoding='utf-8')
            shutil.copyfile(bundle / 'THIRD_PARTY_NOTICES.md', root / 'THIRD_PARTY_NOTICES.md')
            metadata = ET.Element('components')
            for module in ('jediterm-core', 'jediterm-ui'):
                p = bundle / (module + '-3.59-sources.jar'); p.write_bytes(b'source fixture')
                component = ET.SubElement(metadata, 'component', group='org.jetbrains.jediterm', name=module, version='3.59')
                artifact = ET.SubElement(component, 'artifact', name=p.name)
                ET.SubElement(artifact, 'sha256', value=sha256(p))
            (root / 'gradle').mkdir()
            ET.ElementTree(metadata).write(root / 'gradle/verification-metadata.xml')
            manifest = ''.join(sha256(p) + '  ' + p.relative_to(bundle).as_posix() + '\n' for p in bundle.rglob('*') if p.is_file())
            (bundle / 'SHA256SUMS').write_text(manifest, encoding='utf-8')
            with patch.object(collector, 'ROOT', root), patch.object(collector, 'license_files', return_value=[]):
                self.assertEqual(10, collector.verify_desktop_notices(bundle))
                p = bundle / '고지/읽어주세요.txt'; p.write_text('변조', encoding='utf-8')
                with self.assertRaises(ValueError): collector.verify_desktop_notices(bundle)
                (bundle / 'SHA256SUMS').write_text(manifest.replace('고지/읽어주세요.txt', 'C:\\고지.txt'), encoding='utf-8')
                with self.assertRaises(ValueError): collector.verify_desktop_notices(bundle)


if __name__ == '__main__':
    unittest.main()
