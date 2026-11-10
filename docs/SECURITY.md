# 보안 모델과 배포 전 확인

현재 Rust 앱의 동작과 검사 범위입니다. 과거 JVM 패키징·보안 검사 수치는 [HISTORY.md](HISTORY.md)에 요약합니다.

## 연결·저장 경계

- 최초 SSH 호스트키는 SHA-256 지문을 사용자가 확인합니다. 저장된 지문 불일치는 인증 전에 차단하며 자동 재시도하지 않습니다.
- Android Keystore, macOS Keychain, Windows CURRENT_USER DPAPI로 자격 증명을 보호합니다.
  Windows는 참조별 entropy·해시 파일명·암호문 교체 저장을 사용합니다. 보호 저장 실패 시 평문으로 대체하지 않습니다.
- 비밀번호·개인키·암호는 실행 중 메모리에 존재합니다. 모든 런타임의 메모리 완전 삭제를 보장하지 않습니다.
- 복구 중 입력을 차단하고 실패한 입력을 자동 재전송하지 않습니다. 연결 종료는 tmux 세션 삭제 명령을 보내지 않습니다.
- SSH 채널 열기·초기 요청과 tmux 명령·화면 캡처·일반 셸 쓰기는 각 15초로 제한합니다.
  제한 시간 초과는 네트워크 오류로 연결을 종료하고 복구합니다. 일부 전송 여부가 불명확한 입력도 실패로 완료하며 재전송하지 않습니다.
  터미널의 정상 유휴 시간에는 적용하지 않습니다. 에이전트 관측은 같은 쓰기 경로를 사용하지만 기존 8초 제한과 관측 실패 시 대체 경로를 유지합니다.
- 원격 출력·사용자 입력·자격 증명을 기본 로그로 기록하지 않습니다. 진단과 수집 보고서에 비밀정보를 포함하지 않습니다.

WebView는 고정 HTTPS appassets origin의 번들만 사용합니다. 외부 탐색·네트워크 요청·file/content 접근·
권한 요청·OSC52 clipboard 접근을 차단하며 bridge는 origin·main frame·타입·크기·대상 세대를 검사합니다.
클립보드는 사용자의 복사·붙여넣기 동작에만 사용하며, 붙여넣기는 텍스트 작성과 같은 크기·대상 세대 검사를 거칩니다.
터미널 출력의 링크는 사용자가 선택 메뉴에서 고를 때만 http/https·2048자 이하를 native에서 다시 검증해 외부 앱으로 엽니다.

Drive 파일 동기화는 호스트·설정의 허용 필드만 공유합니다. 비밀번호·개인키·키 암호·credentialRef·
신뢰 지문·매크로·터미널 기록은 제외합니다. 주소·사용자명은 공유 파일에 평문으로 남으며 별도 종단간 암호화는 없습니다.
가져온 호스트의 인증은 로컬에서 다시 등록합니다. [동기화 범위](CLOUD_SYNC.md)를 따릅니다.

## 저장소와 CI

`scripts/scan_secrets.py`는 Gitleaks로 Git 관리 작업 파일·전체 staged snapshot·refs 이력을 검사합니다.
탐지 값은 출력하지 않습니다. 임시 SSH 키·설정·로그·빌드 출력은 Git에서 제외합니다.
CI는 `contents: read`, Action SHA 고정, `persist-credentials: false`를 사용합니다.
`pull_request_target`이나 자동 release 게시는 사용하지 않습니다. 산출물은 지정 APK·검사 보고서,
수동 Windows workflow의 ZIP·manifest·해시로 제한합니다.

Wrapper 배포/JAR checksum, Gradle verification metadata·dependency lock, Cargo.lock, npm lock을 유지합니다.
고정 값을 바꿀 때 출처와 변경 이유를 검토하며 검증 실패를 피하기 위한 자동 재생성은 하지 않습니다.
원문 요구사항은 `check_prompt.py`, Wrapper는 `verify_wrapper.py`로 검사합니다.

## 의존성·라이선스·lint

`writeDependencyInventory`는 Gradle의 실제 선택 버전·artifact 해시·사용 범위와 그래프를 수집합니다.
`audit_dependencies.py`는 이 inventory, `Cargo.lock`의 crates.io 패키지, npm lock을 OSV에 대조합니다.
필수 그래프 누락·오래된 입력·해시 오류·crates.io 외 Cargo 출처·OSV 통신 실패는 실패로 처리합니다.
Rust runtime 범위는 배포 타깃의 실제 빌드 기능으로 링크되는 일반 의존성입니다. runtime 권고 예외는 호환되는 수정판이 없는
이유(`blockedBy`)·근거·완화 조건·정확한 사용 범위를 갖춰야 하며, 모든 예외는 90일 안에 만료되어 재검토됩니다.
과거 검사 결과의 패키지 수·0건 기록을 현재 버전의 결과로 재사용하지 않습니다.

별도 [Dependency security audit](../.github/workflows/security-audit.yml)는 수동 실행(`workflow_dispatch`)만 사용합니다.
Gradle inventory를 새로 생성하고 Rust·Maven·npm을 감사하며 APK 패키징·실기 검사는 수행하지 않습니다.
통신 실패·불완전한 inventory·미검토 알림·재검토 기한 경과/만료 예외는 실패 처리하고 inventory·감사 보고서를 14일 보관합니다.
통신·입력 오류로 끝난 보고서는 `complete: false`, `status: failed`와 실패 원인을 남깁니다.

SSH 엔진은 `ring`·RSA 기능을 유지한 `russh =0.65.0`, Windows 간접 의존성은 `pageant 0.2.5`입니다.
[상류 릴리스](https://github.com/Eugeny/russh/releases/tag/v0.65.0)의 중첩 인증서 파싱·EXT_INFO 무제한 할당·
최대 패킷 크기 0에 의한 쓰기 정지 수정과 pageant 프레임 길이 수정을 포함합니다.
기본 로컬 회귀 검사는 `ssh-key` 직접 파싱이 아닌 russh의 보호된 파싱 진입점에서 중첩 인증서 거부를 확인합니다.

남은 [RSA 권고](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)는 수정판 `>=0.10.0`이 있지만
`russh 0.65.0`의 `rsa =0.10.0-rc.18` 고정과 호환되지 않습니다. 기존 RSA 키 지원을 유지하고 강제로 교체하지 않습니다.
DGL 생성 키는 Ed25519이며 RSA 개인키는 사용자가 가져온 키 인증에 쓰입니다. 상류 고정 버전 갱신을 추적합니다.
`pin-utils`는 `gpui → gpui_http_client → zed-async-tar → async-std`의 데스크톱 간접 의존성으로,
[유지보수 중단 공지](https://rustsec.org/advisories/RUSTSEC-2026-0441.html)를 한시적으로 예외 처리합니다.
예외의 재검토일 2026-12-15·만료일 2027-01-27은 연장하지 않습니다.

`verify_licenses.py`는 선택 inventory와 `licenses/dependencies.json`의 원문 해시·고지를 대조합니다.
`refresh_licenses.py`는 검토를 위한 유지보수 도구이며 자동 승인 단계가 아닙니다.
Rust 패키징은 `collect_rust_notices.py`로 Cargo registry의 고지·manifest·DEPENDENCIES.json을 수집합니다.
라이선스 텍스트가 없는 항목은 빈 `licenseFiles`로 드러나며 공개 릴리스 감사 완료를 뜻하지 않습니다.
JediTerm·JVM 고지는 참조 테스트의 의존성 자료이며 Rust 앱이 JVM을 배포한다는 의미가 아닙니다.
[제3자 고지](../THIRD_PARTY_NOTICES.md)를 따릅니다.

lint 검토는 `security/lint-reviews.json`에서 진단 ID·메시지 해시·위치·조건·재검토/만료일을 관리합니다.
`verify_lint.py`는 변경·미검토·만료된 진단을 실패 처리합니다. 경고 수는 실제 실행 기록에만 기재합니다.

재현 명령은 [개발 안내](DEVELOPMENT.md)에 있습니다. 기본 CI와 수동 배포 검사의 범위는 다르며,
기본 CI 성공을 서명·실기·공개 배포 완료로 해석하지 않습니다.

## 남은 확인

- macOS 실제 Keychain 저장·재기동, Windows DPAPI round trip·다른 사용자 격리·변조/키 손실 검사.
- Windows ZIP 실행, 실제 기기·IME·OS 알림, 현재 Rust/Gradle/npm 그래프의 배포 전 감사.
- 서명된 실제 전달물의 고지·키/개인 설정/터미널 기록 제외 확인.
- GitHub Secret scanning·Push protection 활성 상태 확인. 현재 문서만으로 활성 여부를 단정하지 않습니다.

Windows 실행 확인은 사용자 담당입니다. [남은 작업](../TASKS.md)과 [Windows 절차](WINDOWS.md)를 따릅니다.
