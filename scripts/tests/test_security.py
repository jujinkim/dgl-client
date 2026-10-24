import copy
from datetime import date
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import audit_dependencies as audit


class AuditTests(unittest.TestCase):
    def package(self, scope='build'):
        return {'ecosystem': 'Maven', 'name': 'g:a', 'version': '2', 'artifacts': [],
                'usages': [{'scope': scope, 'module': ':', 'configuration': 'classpath', 'platform': 'build-host'}]}

    def exception(self):
        p = self.package()
        return dict(p, id='GHSA-example', reviewed='2026-10-01', recheck='2026-11-01', expires='2026-12-01',
                    reason='Fixture', evidence=['https://example.test/advisory'], conditions=['Fixture'])

    def test_selected_versions_only_and_scope_merge(self):
        packages = audit.combine([self.package(), self.package('test')])
        sent = []
        def request(url, data):
            sent.extend(data['queries'])
            return {'results': [{'vulns': [{'id': 'GHSA-example'}, {'id': 'GHSA-example'}]}]}
        findings = audit.query_osv(packages, request)
        self.assertEqual(['2'], [q['version'] for q in sent])
        self.assertEqual(1, len(findings))
        self.assertEqual({'build', 'test'}, {u['scope'] for u in findings[0]['usages']})

    def test_missing_result_and_error_fail(self):
        for response in [{'results': []}, {}, {'results': [{'error': 'failed'}]}, {'results': [None]},
                         {'results': [{'vulns': [{'modified': 'today'}]}]}, {'results': [{'next_page_token': 'more'}]}]:
            with self.subTest(response=response), self.assertRaises(ValueError):
                audit.query_osv([self.package()], lambda *args: response)

    def test_transport_error_propagates(self):
        def fail(*args):
            raise OSError('offline')
        with self.assertRaises(OSError):
            audit.query_osv([self.package()], fail)

    def test_runtime_exception_rejected(self):
        e = self.exception()
        with self.assertRaises(ValueError):
            audit.apply_exceptions([dict(self.package('runtime'), id=e['id'])], [e], date(2026, 10, 23))
        e['usages'][0]['scope'] = 'runtime'
        with self.assertRaises(ValueError):
            audit.apply_exceptions([], [e], date(2026, 10, 23))

    def test_expired_overlong_or_unsubstantiated_rejected(self):
        for change in [{'expires': '2026-10-22'}, {'expires': '2027-01-01'}, {'recheck': '2026-10-22'},
                       {'reason': ''}, {'evidence': []}, {'conditions': []}]:
            with self.subTest(change=change), self.assertRaises(ValueError):
                audit.apply_exceptions([], [dict(self.exception(), **change)], date(2026, 10, 23))

    def test_precise_exception_and_uncovered_usage(self):
        e = self.exception()
        findings = audit.apply_exceptions([dict(self.package(), id=e['id'])], [e], date(2026, 10, 23))
        self.assertEqual('excepted', findings[0]['status'])
        findings[0]['usages'].append(dict(findings[0]['usages'][0], module=':another'))
        with self.assertRaises(ValueError):
            audit.apply_exceptions(findings, [e], date(2026, 10, 23))
        self.assertEqual('blocked', audit.apply_exceptions([dict(self.package(), id='different')], [e], date(2026, 10, 23))[0]['status'])

    def test_npm_nested_alias_and_dev_scope(self):
        lock = {'lockfileVersion': 3, 'packages': {'': {},
            'node_modules/a/node_modules/@scope/alias': {'name': '@scope/real', 'version': '1', 'dev': True,
                                                       'integrity': 'sha512-test', 'resolved': 'https://example.test/pkg'}}}
        p = audit.npm_packages(lock)[0]
        self.assertEqual('@scope/real', p['name'])
        self.assertEqual('build', p['usages'][0]['scope'])
        del lock['packages']['node_modules/a/node_modules/@scope/alias']['name']
        self.assertEqual('@scope/alias', audit.npm_packages(lock)[0]['name'])

    def test_empty_inventory_rejected(self):
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'inventory.json'
            for data in [{}, {'schema': 1, 'complete': True, 'packages': [], 'inputs': {}}]:
                path.write_text(json.dumps(data), encoding="utf-8")
                with self.assertRaises(ValueError):
                    audit.load_inventory(path)


class LicenseTests(unittest.TestCase):
    def test_missing_license_and_native_notices_rejected(self):
        import verify_licenses as licenses
        package = AuditTests().package('runtime')
        for registry in [{'schema': 1, 'packages': [], 'native': []},
                         {'schema': 1, 'packages': [dict(package, licenses=[], evidence=[])], 'native': []}]:
            with self.assertRaises(ValueError):
                licenses.verify([package], registry)

    def test_missing_or_modified_license_file_rejected(self):
        import verify_licenses as licenses
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            record = {'path': 'LICENSE', 'sha256': '0' * 64}
            with self.assertRaises(ValueError):
                licenses.check_file(record, root)
            (root / 'LICENSE').write_text('Modified license', encoding="utf-8")
            with self.assertRaises(ValueError):
                licenses.check_file(record, root)


class InventoryTests(unittest.TestCase):
    def test_reference_dependency_scopes_and_incomplete_graph(self):
        import tempfile
        import hashlib
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'build.gradle.kts').write_text('fixture', encoding="utf-8")
            graphs = [{'module': m, 'configuration': c, 'platform': 'android', 'scope': 'runtime', 'edges': []}
                      for m, c in sorted(audit.REQUIRED_GRAPHS)]
            reference = {'module': ':app', 'configuration': 'referenceTest', 'platform': 'host-desktop',
                         'scope': 'runtime', 'edges': [{'to': 'com.hierynomus:sshj:0.40.0'}]}
            graphs.append(reference)
            package = {'ecosystem': 'Maven', 'name': 'com.hierynomus:sshj', 'version': '0.40.0',
                       'artifacts': [{'name': 'sshj.jar', 'sha256': 'a'*64}],
                       'usages': [dict(reference, selected='com.hierynomus:sshj:0.40.0')]}
            data = {'schema': 1, 'complete': True, 'packages': [package], 'graphs': graphs,
                    'inputs': {'build.gradle.kts': hashlib.sha256(b'fixture').hexdigest()}}
            path = root / 'inventory.json'
            path.write_text(json.dumps(data), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, 'Retired JVM dependency'):
                audit.load_inventory(path, root)
            reference['scope'] = package['usages'][0]['scope'] = 'test'
            path.write_text(json.dumps(data), encoding="utf-8")
            self.assertEqual(1, len(audit.load_inventory(path, root)))
            data['graphs'].pop()
            path.write_text(json.dumps(data), encoding="utf-8")
            with self.assertRaises(ValueError):
                audit.load_inventory(path, root)


class LintTests(unittest.TestCase):
    def test_changed_and_expired_diagnostics_fail(self):
        import verify_lint as lint
        import xml.etree.ElementTree as ET
        import hashlib
        report = ET.fromstring('<issues><issue id="Review" severity="Warning" message="fixed"><location file="/root/source.kt"/></issue></issues>')
        review = {'id': 'Review', 'messageSha256': hashlib.sha256(b'fixed').hexdigest(), 'locationSuffix': 'source.kt',
                  'reviewed': '2026-10-23', 'recheck': '2026-11-22', 'expires': '2026-12-22',
                  'reason': 'Fixture', 'evidence': ['source.kt'], 'condition': 'Fixture'}
        self.assertEqual(1, lint.verify(report, [review], date(2026, 10, 23)))
        with self.assertRaises(ValueError):
            lint.verify(report, [review], date(2026, 12, 23))
        report.find('issue').set('message', 'new warning')
        with self.assertRaises(ValueError):
            lint.verify(report, [review], date(2026, 10, 23))


if __name__ == '__main__':
    unittest.main()
