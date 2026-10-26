import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collect_artifacts as collector
from evidence import ROOT, safe_relative, sha256
from verify_android_results import EXPECTED_ANDROID, expected_for_selection, verify as verify_android
from verify_test_results import EXPECTED, verify as verify_jvm


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
            ssh_tests = sum(any('.' + cls + '.' in name for cls in
                                ('IntegrationTest', 'RecoveryTest', 'PerformanceTest')) for name in EXPECTED)
            self.assertEqual(len(EXPECTED) - ssh_tests, verify_jvm(root, 'windows')['passed'])
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
        summary = f'OK ({len(EXPECTED_ANDROID)} tests)'
        output += summary + '\n'
        self.assertEqual(len(EXPECTED_ANDROID), verify_android(output)['passed'])
        for broken in (output.replace('CODE: 0', 'CODE: -3', 1), summary, output.replace(summary, 'OK (1 tests)')):
            with self.assertRaises(ValueError): verify_android(broken)

class TestInventoryTests(unittest.TestCase):
    def test_named_inventories_match_current_test_sources(self):
        # Catch new tests omitted from the strict CI result inventory.
        for roots, expected in ((['app/src/commonTest', 'app/src/desktopTest'], EXPECTED),
                                (['androidApp/src/androidTest'], EXPECTED_ANDROID)):
            actual = set()
            for directory in roots:
                for path in (ROOT / directory).rglob('*.kt'):
                    source = path.read_text(encoding='utf-8')
                    methods = re.findall(r'@Test\s+(?:suspend\s+)?fun\s+(\w+)\s*\(', source)
                    if methods:
                        package = re.search(r'^package (\S+)', source, re.M).group(1)
                        actual.update(package + '.' + path.stem + '.' + method for method in methods)
            self.assertEqual(expected, actual)

    def test_selected_instrumentation_requires_each_named_result(self):
        name = 'kim.jujin.dglc.AndroidHierarchyTest.compactRowsExpansionPanelDetailsAndSelectedHostActions'
        cls, method = name.rsplit('.', 1)
        self.assertEqual({name}, expected_for_selection(cls))
        self.assertEqual({name}, expected_for_selection(cls + '#' + method))
        output = (f'INSTRUMENTATION_STATUS: class={cls}\nINSTRUMENTATION_STATUS: test={method}\n'
                  'INSTRUMENTATION_STATUS_CODE: 0\nOK (1 test)\n')
        self.assertEqual(1, verify_android(output, {name})['passed'])
        for invalid in (output.replace(method, 'unrelatedTest'), output.replace('CODE: 0', 'CODE: -3'),
                        'OK (1 test)', output + output):
            with self.assertRaises(ValueError):
                verify_android(invalid, {name})
        for invalid in ('', 'UnknownTest', cls + '#missing', cls + ',UnknownTest'):
            with self.assertRaises(ValueError):
                expected_for_selection(invalid)


if __name__ == '__main__':
    unittest.main()
