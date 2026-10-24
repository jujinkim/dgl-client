# 작업 지침

## 브랜치 운영

- 2026-10-23 사용자 요청에 따라 당분간 `main` 브랜치만 운영한다.
- 사용자가 브랜치 운영 방식을 변경하기 전까지 구현·수정·커밋은 `main`에서 진행하고, 새 작업 브랜치를 만들지 않는다.
- 과거 작업 기록에 있는 전용 브랜치 안내보다 이 지침을 우선한다.

## Windows 빌드 환경 유지

- Windows 실기 검증을 이번 작업에서 제외하더라도 Windows 빌드·패키징 환경은 유지한다. 검증 범위 제외를 플랫폼 구현·빌드 지원 제외로 해석하지 않는다.
- Rust 데스크톱을 수정할 때 macOS 전용 의존성·경로·API를 공통 코드에 무조건 적용하지 않고 Windows MSVC x64 빌드 경로를 함께 고려한다.
- Windows 빌드 진입점은 `scripts/build_windows.ps1`, 패키징은 `scripts/package_rust_windows.py`, 수동 CI는 `.github/workflows/windows-build.yml`이다. 절차·필수 도구·출력 경로가 바뀌면 `docs/WINDOWS.md`와 README도 함께 갱신한다.
- 기본 Windows 스크립트와 수동 CI는 빌드·ZIP 생성만 수행한다. 테스트는 `-Test`로 별도 선택하며, Windows 실기 검증은 사용자 담당으로 둔다. 실제 수행하지 않은 Windows 네이티브 빌드·DPAPI·GUI 검증을 통과로 기록하지 않는다.

## 대시보드 등록 유지

- 프로젝트 속성·앱 ID·문서·빌드 출력 경로·파일별 버전·LAN URL·배포 방식이 바뀌면 `.dashboard-props.json`을 갱신한다. 설명은 앱 목적과 핵심 기능을 담은 한 문장(가급적 120자 이내)으로 유지하고 작업 진행·이력을 덧붙이지 않는다.
- `/Users/jujin/workspace/dashboard/DASHBOARD_GUIDE.md`를 따르고 수정 후 대시보드 검증기를 실행한다. JSON은 Git 제외 상태를 유지하며 비밀정보를 기록하거나 서명·설치 가능 여부를 추측하지 않는다.
