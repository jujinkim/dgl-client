#!/usr/bin/env python3
"""Fail on unreviewed, changed or overdue lint diagnostics; never hide warnings."""
from datetime import date
import hashlib
import json
from pathlib import Path
import sys
import xml.etree.ElementTree as ET
from audit_dependencies import ROOT


def verify(report, reviews, today=None):
    today = today or date.today()
    for r in reviews:
        reviewed, recheck, expires = (date.fromisoformat(r[k]) for k in ('reviewed', 'recheck', 'expires'))
        if not reviewed <= today <= recheck <= expires or not 0 < (expires-reviewed).days <= 90:
            raise ValueError('Lint review expired or overdue: ' + r['id'])
        if not all(r.get(k) for k in ('reason', 'evidence', 'condition', 'locationSuffix', 'messageSha256')):
            raise ValueError('Incomplete lint review')
    count = 0
    for issue in report.findall('issue'):
        if issue.attrib['severity'] in {'Error', 'Fatal'}:
            raise ValueError('Lint error: ' + issue.attrib['id'])
        sha = hashlib.sha256(issue.attrib['message'].encode()).hexdigest()
        for location in issue.findall('location'):
            path = location.attrib['file'].replace('\\', '/')
            if not any(r['id'] == issue.attrib['id'] and r['messageSha256'] == sha and path.endswith(r['locationSuffix']) for r in reviews):
                raise ValueError('Unreviewed lint diagnostic: ' + issue.attrib['id'])
        if not issue.findall('location'):
            raise ValueError('Lint diagnostic without location')
        count += 1
    return count


if __name__ == '__main__':
    try:
        count = verify(ET.parse(ROOT / 'androidApp/build/reports/lint-results-debug.xml'),
                       json.loads((ROOT / 'security/lint-reviews.json').read_text(encoding="utf-8"))['reviews'])
        print(f'Lint: {count} exact diagnostics reviewed; recheck 2026-11-22, expiry 2026-12-22')
    except (OSError, ValueError, KeyError) as error:
        print('Lint review FAILED: ' + str(error), file=sys.stderr)
        sys.exit(1)
