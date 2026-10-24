//! Native WinRT toast, registered to dgl's own user Start Menu shortcut (ZIP installs).
use super::*;
use ::windows::{
    Data::Xml::Dom::XmlDocument,
    UI::Notifications::{ToastNotification, ToastNotificationManager},
    Win32::{
        Foundation::PROPERTYKEY,
        System::Com::{
            CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, StructuredStorage::PROPVARIANT,
        },
        UI::Shell::{
            IShellLinkW, PropertiesSystem::IPropertyStore, SetCurrentProcessExplicitAppUserModelID,
            ShellLink,
        },
    },
    core::{GUID, HSTRING, Interface},
};
use std::collections::VecDeque;
thread_local! { static TOASTS: std::cell::RefCell<VecDeque<ToastNotification>> = const { std::cell::RefCell::new(VecDeque::new()) }; }
const ID: &str = "kim.jujin.dgl.rust-candidate";
const ACTIVATOR: GUID = GUID::from_u128(0x5fd514f1_b83f_4f6d_a85b_156ad0e79e33);
const ACTIVATOR_TEXT: &str = "{5FD514F1-B83F-4F6D-A85B-156AD0E79E33}";
use ::windows::{
    Win32::{
        Foundation::{CLASS_E_NOAGGREGATION, E_POINTER},
        System::{
            Com::{
                CLSCTX_LOCAL_SERVER, CoRegisterClassObject, IClassFactory, IClassFactory_Impl,
                REGCLS_MULTIPLEUSE, StructuredStorage::InitPropVariantFromCLSID,
            },
            Registry::{
                HKEY, HKEY_CURRENT_USER, REG_SZ, RegCloseKey, RegCreateKeyW, RegSetValueExW,
            },
        },
        UI::Notifications::{
            INotificationActivationCallback, INotificationActivationCallback_Impl,
            NOTIFICATION_USER_INPUT_DATA,
        },
    },
    core::{BOOL, IUnknown, PCWSTR, Ref, implement},
};
#[implement(INotificationActivationCallback)]
struct Activator;
impl INotificationActivationCallback_Impl for Activator_Impl {
    fn Activate(
        &self,
        app: &PCWSTR,
        args: &PCWSTR,
        _: *const NOTIFICATION_USER_INPUT_DATA,
        _: u32,
    ) -> ::windows::core::Result<()> {
        if unsafe { app.to_string()? } == ID {
            let id = unsafe { args.to_string()? };
            if id.len() <= 1024 {
                clicked(id);
            }
        }
        Ok(())
    }
}
#[implement(IClassFactory)]
struct Factory;
impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<'_, IUnknown>,
        iid: *const GUID,
        out: *mut *mut core::ffi::c_void,
    ) -> ::windows::core::Result<()> {
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        if iid.is_null() || out.is_null() {
            return Err(E_POINTER.into());
        }
        let callback: INotificationActivationCallback = Activator.into();
        unsafe {
            *out = core::ptr::null_mut();
            callback.query(iid, out).ok()
        }
    }
    fn LockServer(&self, _: BOOL) -> ::windows::core::Result<()> {
        Ok(())
    }
}
fn registry_string(path: &str, name: &str, value: &str) -> Option<()> {
    let bytes: Vec<u8> = value
        .encode_utf16()
        .chain(Some(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyW(HKEY_CURRENT_USER, &HSTRING::from(path), &mut key)
            .ok()
            .ok()?;
        let result = RegSetValueExW(key, &HSTRING::from(name), None, REG_SZ, Some(&bytes)).ok();
        let _ = RegCloseKey(key);
        result.ok()
    }
}

pub fn initialize() -> bool {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    *REGISTERED.get_or_init(|| unsafe {
        if SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(ID)).is_err() {
            return false;
        }
        let factory: IClassFactory = Factory.into();
        CoRegisterClassObject(
            &ACTIVATOR,
            &factory,
            CLSCTX_LOCAL_SERVER,
            REGCLS_MULTIPLEUSE,
        )
        .is_ok()
    })
}
fn register() -> Option<()> {
    let exe = std::env::current_exe().ok()?;
    let directory = std::path::PathBuf::from(std::env::var_os("APPDATA")?)
        .join("Microsoft/Windows/Start Menu/Programs");
    std::fs::create_dir_all(&directory).ok()?;
    let path = directory.join("dgl Rust Candidate.lnk");
    registry_string(
        &format!("Software\\Classes\\CLSID\\{ACTIVATOR_TEXT}\\LocalServer32"),
        "",
        &format!("\"{}\" --agent-toast", exe.display()),
    )?;
    registry_string(
        &format!("Software\\Classes\\AppUserModelId\\{ID}"),
        "DisplayName",
        "dgl Rust Candidate",
    )?;
    registry_string(
        &format!("Software\\Classes\\AppUserModelId\\{ID}"),
        "CustomActivator",
        ACTIVATOR_TEXT,
    )?;
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        link.SetPath(&HSTRING::from(exe.as_os_str())).ok()?;
        link.SetDescription(&HSTRING::from("dgl SSH tmux client"))
            .ok()?;
        let props: IPropertyStore = link.cast().ok()?;
        let key = PROPERTYKEY {
            fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
            pid: 5,
        };
        props.SetValue(&key, &PROPVARIANT::from(ID)).ok()?;
        let activator_key = PROPERTYKEY {
            fmtid: key.fmtid,
            pid: 26,
        };
        props
            .SetValue(&activator_key, &InitPropVariantFromCLSID(&ACTIVATOR).ok()?)
            .ok()?;
        props.Commit().ok()?;
        let persist: IPersistFile = link.cast().ok()?;
        persist.Save(&HSTRING::from(path.as_os_str()), true).ok()?;
    }
    Some(())
}
pub fn authorize() -> bool {
    initialize() && register().is_some()
}
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
pub fn show(notice: &Notice) -> bool {
    let result = (|| -> ::windows::core::Result<()> {
        let doc = XmlDocument::new()?;
        let title = format!(
            "{:?} · {}{}",
            notice.event.agent,
            notice.event.pane,
            if notice.event.actor != "parent" {
                " · 서브에이전트"
            } else {
                ""
            }
        );
        doc.LoadXml(&HSTRING::from(format!("<toast launch=\"{}\"><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>", xml(&notice.id()), xml(&title), xml(notice.event.kind.label()))))?;
        let toast = ToastNotification::CreateToastNotification(&doc)?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(ID))?.Show(&toast)?;
        TOASTS.with(|toasts| {
            let mut t = toasts.borrow_mut();
            t.push_back(toast);
            while t.len() > 100 {
                t.pop_front();
            }
        });
        Ok(())
    })();
    result.is_ok()
}
