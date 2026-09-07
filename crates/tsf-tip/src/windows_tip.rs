use std::ffi::c_void;
use std::io::Write;
use std::mem::ManuallyDrop;
use std::ptr::{null, null_mut};
use std::slice;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::core::{
    ComInterface, Error, IUnknown, IUnknown_Vtbl, Interface, GUID, HRESULT, HSTRING, PCWSTR,
};
use windows::Win32::Foundation::{
    CloseHandle, BOOL, CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, ERROR_FILE_NOT_FOUND,
    E_NOINTERFACE, E_POINTER, HMODULE, LPARAM, S_FALSE, S_OK, WAIT_ABANDONED, WAIT_OBJECT_0,
    WPARAM,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IClassFactory, IClassFactory_Vtbl,
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use windows::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_EXPAND_SZ,
    REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_F6;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, ITfCategoryMgr, ITfCompositionSink,
    ITfCompositionSink_Vtbl, ITfContext, ITfContextComposition, ITfEditSession,
    ITfEditSession_Vtbl, ITfInputProcessorProfileMgr, ITfInputProcessorProfiles, ITfKeyEventSink,
    ITfKeyEventSink_Vtbl, ITfKeystrokeMgr, ITfTextInputProcessor, ITfTextInputProcessorEx,
    ITfTextInputProcessorEx_Vtbl, ITfTextInputProcessor_Vtbl, ITfThreadMgr,
    GUID_TFCAT_TIP_KEYBOARD, HKL, TF_DEFAULT_SELECTION, TF_ES_READWRITE, TF_ES_SYNC,
    TF_INPUTPROCESSORPROFILE, TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE, TF_IPPMF_FORSESSION,
    TF_IPP_FLAG_ACTIVE, TF_IPP_FLAG_ENABLED, TF_LANGUAGEPROFILE, TF_PROFILETYPE_INPUTPROCESSOR,
    TF_PROFILETYPE_KEYBOARDLAYOUT, TF_SELECTION, TF_ST_CORRECTION,
};

pub const TIP_DESCRIPTION: &str = "Doubao Voice Input";
pub const TIP_CLSID: GUID = GUID::from_u128(0x8f5c8c59_2a4d_4ddf_8ebf_f2ab0e9b5a31);
pub const TIP_PROFILE_GUID: GUID = GUID::from_u128(0x29d0f4f7_4e0c_45fd_b287_d1e9fd9aa8d4);
pub const TIP_LANGID: u16 = 0x0804; // zh-CN

const THREADING_MODEL: &str = "Apartment";
const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x80010106u32 as i32);
const E_FAIL: HRESULT = HRESULT(0x80004005u32 as i32);
const LOG_MUTEX_NAME: &str = "Local\\DoubaoVoiceInputTsfTipLogMutex";
const LOG_MUTEX_TIMEOUT_MS: u32 = 2000;
const PROFILE_SWITCH_SETTLE_MS: u64 = 800;

static OBJECT_COUNT: AtomicU32 = AtomicU32::new(0);
static SERVER_LOCK_COUNT: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone)]
pub struct TipRegistrationStatus {
    pub clsid: String,
    pub profile_guid: String,
    pub langid: u16,
    pub description: &'static str,
    pub com_key_present: bool,
    pub com_dll_path: Option<String>,
    pub threading_model: Option<String>,
    pub tsf_profile_key_present: bool,
    pub tsf_profile_registered: bool,
    pub tsf_profile_enabled: Option<bool>,
    pub keyboard_category_registered: bool,
    pub tsf_profile_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ActiveProfileSummary {
    pub profile_type: u32,
    pub langid: u16,
    pub clsid: String,
    pub profile_guid: String,
    pub category: String,
    pub hkl: isize,
    pub enabled: bool,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct ProfileSwitchSmokeTestResult {
    pub before: ActiveProfileSummary,
    pub restore_target: ActiveProfileSummary,
    pub after_doubao: ActiveProfileSummary,
    pub after_restore: ActiveProfileSummary,
}

#[repr(C)]
struct ClassFactoryObject {
    vtbl: *const IClassFactory_Vtbl,
    ref_count: AtomicU32,
}

#[repr(C)]
struct TextServiceObject {
    vtbl: *const ITfTextInputProcessorEx_Vtbl,
    ref_count: AtomicU32,
    client_id: AtomicU32,
    activate_flags: AtomicU32,
    thread_mgr: Mutex<Option<ITfThreadMgr>>,
}

#[repr(C)]
struct KeyEventSinkObject {
    vtbl: *const ITfKeyEventSink_Vtbl,
    ref_count: AtomicU32,
    service: *mut TextServiceObject,
}

#[repr(C)]
struct EditSessionObject {
    vtbl: *const ITfEditSession_Vtbl,
    ref_count: AtomicU32,
    service: *mut TextServiceObject,
    context: ITfContext,
}

#[repr(C)]
struct CompositionSinkObject {
    vtbl: *const ITfCompositionSink_Vtbl,
    ref_count: AtomicU32,
    service: *mut TextServiceObject,
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    tracing::info!("DllGetClassObject called");
    diagnostic_log("DllGetClassObject called");

    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = null_mut();

    if rclsid.is_null() || riid.is_null() {
        return E_POINTER;
    }

    if *rclsid != TIP_CLSID {
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    let factory = Box::into_raw(ClassFactoryObject::new()).cast::<c_void>();
    let hr = class_factory_query_interface(factory, riid, ppv);
    class_factory_release(factory);
    hr
}

#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECT_COUNT.load(Ordering::SeqCst) == 0 && SERVER_LOCK_COUNT.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[no_mangle]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    let result = register_server();
    if let Err(error) = &result {
        tracing::error!(?error, "DllRegisterServer failed");
        diagnostic_log(format!("DllRegisterServer failed: {error:?}"));
    }
    result.into()
}

#[no_mangle]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    let result = unregister_server();
    if let Err(error) = &result {
        tracing::error!(?error, "DllUnregisterServer failed");
        diagnostic_log(format!("DllUnregisterServer failed: {error:?}"));
    }
    result.into()
}

fn register_server() -> windows::core::Result<()> {
    let dll_path = current_dll_path()?;
    register_server_with_path(&dll_path)
}

pub fn register_server_with_path(dll_path: &str) -> windows::core::Result<()> {
    register_com_server(dll_path)?;
    register_tsf_profile(dll_path)?;
    register_tsf_categories()?;
    tracing::info!(dll_path, "Doubao TSF TIP registered");
    diagnostic_log(format!("Doubao TSF TIP registered: {dll_path}"));
    Ok(())
}

pub fn unregister_server() -> windows::core::Result<()> {
    let category_result = unregister_tsf_categories();
    let profile_result = unregister_tsf_profile();
    let registry_result = unregister_com_server();

    category_result?;
    profile_result?;
    registry_result?;

    tracing::info!("Doubao TSF TIP unregistered");
    diagnostic_log("Doubao TSF TIP unregistered");
    Ok(())
}

pub fn query_registration_status() -> TipRegistrationStatus {
    let clsid = guid_with_braces(&TIP_CLSID);
    let profile_guid = guid_with_braces(&TIP_PROFILE_GUID);
    let clsid_key_path = clsid_key_path();
    let tsf_profile_key_path = tsf_profile_key_path();

    let (com_key_present, com_dll_path, threading_model) = match RegKey::open(
        HKEY_CLASSES_ROOT,
        &format!("{clsid_key_path}\\InProcServer32"),
    ) {
        Ok(key) => (
            true,
            key.query_string(None).ok().flatten(),
            key.query_string(Some("ThreadingModel")).ok().flatten(),
        ),
        Err(error) if is_file_not_found(&error) => (false, None, None),
        Err(error) => (false, None, Some(format!("registry error: {error:?}"))),
    };

    let tsf_profile_key_present = match RegKey::open(HKEY_LOCAL_MACHINE, &tsf_profile_key_path) {
        Ok(_) => true,
        Err(error) if is_file_not_found(&error) => false,
        Err(_) => false,
    };

    let (tsf_profile_registered, tsf_profile_enabled, tsf_profile_error) =
        match query_tsf_profile_state() {
            Ok((registered, enabled)) => (registered, enabled, None),
            Err(error) => (false, None, Some(format!("{error:?}"))),
        };
    let keyboard_category_registered = query_keyboard_category_registered().unwrap_or(false);

    TipRegistrationStatus {
        clsid,
        profile_guid,
        langid: TIP_LANGID,
        description: TIP_DESCRIPTION,
        com_key_present,
        com_dll_path,
        threading_model,
        tsf_profile_key_present,
        tsf_profile_registered,
        tsf_profile_enabled,
        keyboard_category_registered,
        tsf_profile_error,
    }
}

pub fn query_active_keyboard_profile() -> windows::core::Result<ActiveProfileSummary> {
    let _com = ComInitGuard::initialize()?;
    let manager = input_processor_profile_mgr()?;
    active_keyboard_profile(&manager).map(ActiveProfileSummary::from)
}

pub fn switch_profile_smoke_test() -> windows::core::Result<ProfileSwitchSmokeTestResult> {
    let _com = ComInitGuard::initialize()?;
    let manager = input_processor_profile_mgr()?;
    let before = active_keyboard_profile(&manager)?;
    let restore_target = if is_doubao_profile(&before) {
        find_restore_profile(&manager)?
    } else {
        before
    };

    activate_doubao_profile(&manager)?;
    std::thread::sleep(Duration::from_millis(PROFILE_SWITCH_SETTLE_MS));
    let after_doubao = active_keyboard_profile(&manager)?;

    activate_profile(&manager, &restore_target)?;
    std::thread::sleep(Duration::from_millis(PROFILE_SWITCH_SETTLE_MS));
    let after_restore = active_keyboard_profile(&manager)?;

    Ok(ProfileSwitchSmokeTestResult {
        before: before.into(),
        restore_target: restore_target.into(),
        after_doubao: after_doubao.into(),
        after_restore: after_restore.into(),
    })
}

fn register_com_server(dll_path: &str) -> windows::core::Result<()> {
    let clsid_key_path = clsid_key_path();
    let clsid_key = RegKey::create(&clsid_key_path)?;
    clsid_key.set_string(None, TIP_DESCRIPTION)?;

    let inproc_key = RegKey::create(&format!("{clsid_key_path}\\InProcServer32"))?;
    inproc_key.set_string(None, dll_path)?;
    inproc_key.set_string(Some("ThreadingModel"), THREADING_MODEL)?;

    Ok(())
}

fn unregister_com_server() -> windows::core::Result<()> {
    let clsid_key_path = clsid_key_path();
    match RegKey::delete_tree(&clsid_key_path) {
        Ok(()) => Ok(()),
        Err(error) if is_file_not_found(&error) => Ok(()),
        Err(error) => Err(error),
    }
}

fn register_tsf_profile(dll_path: &str) -> windows::core::Result<()> {
    let _com = ComInitGuard::initialize()?;
    let profiles = input_processor_profiles()?;
    let description = wide_z(TIP_DESCRIPTION);
    let icon_file = wide_z(dll_path);

    unsafe {
        profiles.Register(&TIP_CLSID)?;

        if let Err(error) =
            profiles.RemoveLanguageProfile(&TIP_CLSID, TIP_LANGID, &TIP_PROFILE_GUID)
        {
            tracing::debug!(
                ?error,
                "RemoveLanguageProfile before registration did not remove an existing profile"
            );
        }

        profiles.AddLanguageProfile(
            &TIP_CLSID,
            TIP_LANGID,
            &TIP_PROFILE_GUID,
            &description,
            &icon_file,
            0,
        )?;
        profiles.EnableLanguageProfile(&TIP_CLSID, TIP_LANGID, &TIP_PROFILE_GUID, BOOL(1))?;
    }

    Ok(())
}

fn unregister_tsf_profile() -> windows::core::Result<()> {
    let _com = ComInitGuard::initialize()?;
    let profiles = input_processor_profiles()?;

    unsafe {
        if let Err(error) =
            profiles.RemoveLanguageProfile(&TIP_CLSID, TIP_LANGID, &TIP_PROFILE_GUID)
        {
            tracing::warn!(?error, "RemoveLanguageProfile failed during cleanup");
        }
        if let Err(error) = profiles.Unregister(&TIP_CLSID) {
            tracing::warn!(
                ?error,
                "ITfInputProcessorProfiles::Unregister failed during cleanup"
            );
        }
    }

    Ok(())
}

fn register_tsf_categories() -> windows::core::Result<()> {
    let _com = ComInitGuard::initialize()?;
    let category_mgr = category_mgr()?;
    unsafe {
        category_mgr.RegisterCategory(&TIP_CLSID, &GUID_TFCAT_TIP_KEYBOARD, &TIP_CLSID)?;
    }
    Ok(())
}

fn unregister_tsf_categories() -> windows::core::Result<()> {
    let _com = ComInitGuard::initialize()?;
    let category_mgr = category_mgr()?;
    unsafe {
        if let Err(error) =
            category_mgr.UnregisterCategory(&TIP_CLSID, &GUID_TFCAT_TIP_KEYBOARD, &TIP_CLSID)
        {
            tracing::warn!(?error, "UnregisterCategory(GUID_TFCAT_TIP_KEYBOARD) failed");
        }
    }
    Ok(())
}

fn query_tsf_profile_state() -> windows::core::Result<(bool, Option<bool>)> {
    let _com = ComInitGuard::initialize()?;
    let profiles = input_processor_profiles()?;
    let language_profiles = unsafe { profiles.EnumLanguageProfiles(TIP_LANGID)? };

    loop {
        let mut profile = [unsafe { std::mem::zeroed::<TF_LANGUAGEPROFILE>() }];
        let mut fetched = 0u32;
        unsafe {
            language_profiles.Next(&mut profile, &mut fetched)?;
        }

        if fetched == 0 {
            return Ok((false, None));
        }

        let profile = profile[0];
        if profile.clsid == TIP_CLSID && profile.guidProfile == TIP_PROFILE_GUID {
            let enabled = unsafe {
                profiles.IsEnabledLanguageProfile(&TIP_CLSID, TIP_LANGID, &TIP_PROFILE_GUID)?
            };
            return Ok((true, Some(enabled.as_bool())));
        }
    }
}

fn query_keyboard_category_registered() -> windows::core::Result<bool> {
    let _com = ComInitGuard::initialize()?;
    let category_mgr = category_mgr()?;
    let items = unsafe { category_mgr.EnumItemsInCategory(&GUID_TFCAT_TIP_KEYBOARD)? };

    loop {
        let mut item = [GUID::zeroed()];
        let mut fetched = 0u32;
        let hr = unsafe { items.Next(&mut item, Some(&mut fetched)) };
        if hr.is_err() || fetched == 0 {
            return Ok(false);
        }
        if item[0] == TIP_CLSID {
            return Ok(true);
        }
    }
}

fn active_keyboard_profile(
    manager: &ITfInputProcessorProfileMgr,
) -> windows::core::Result<TF_INPUTPROCESSORPROFILE> {
    let mut profile = TF_INPUTPROCESSORPROFILE::default();
    unsafe {
        manager.GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut profile)?;
    }
    Ok(profile)
}

fn find_restore_profile(
    manager: &ITfInputProcessorProfileMgr,
) -> windows::core::Result<TF_INPUTPROCESSORPROFILE> {
    for langid in [TIP_LANGID, 0x0409, 0] {
        let profiles = match unsafe { manager.EnumProfiles(langid) } {
            Ok(profiles) => profiles,
            Err(_) => continue,
        };

        loop {
            let mut profile = [TF_INPUTPROCESSORPROFILE::default()];
            let mut fetched = 0u32;
            unsafe {
                profiles.Next(&mut profile, &mut fetched)?;
            }
            if fetched == 0 {
                break;
            }

            let profile = profile[0];
            let supported_profile_type = profile.dwProfileType == TF_PROFILETYPE_INPUTPROCESSOR
                || profile.dwProfileType == TF_PROFILETYPE_KEYBOARDLAYOUT;
            let enabled = profile.dwFlags & TF_IPP_FLAG_ENABLED != 0;
            if supported_profile_type && enabled && !is_doubao_profile(&profile) {
                return Ok(profile);
            }
        }
    }

    Err(Error::new(
        E_FAIL,
        HSTRING::from("active profile is already Doubao and no enabled restore profile was found"),
    ))
}

fn activate_doubao_profile(manager: &ITfInputProcessorProfileMgr) -> windows::core::Result<()> {
    unsafe {
        manager.ActivateProfile(
            TF_PROFILETYPE_INPUTPROCESSOR,
            TIP_LANGID,
            &TIP_CLSID,
            &TIP_PROFILE_GUID,
            HKL(0),
            TF_IPPMF_FORSESSION | TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE,
        )
    }
}

fn activate_profile(
    manager: &ITfInputProcessorProfileMgr,
    profile: &TF_INPUTPROCESSORPROFILE,
) -> windows::core::Result<()> {
    let flags = TF_IPPMF_FORSESSION | TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE;
    unsafe {
        match profile.dwProfileType {
            TF_PROFILETYPE_INPUTPROCESSOR => manager.ActivateProfile(
                profile.dwProfileType,
                profile.langid,
                &profile.clsid,
                &profile.guidProfile,
                profile.hklSubstitute,
                flags,
            ),
            TF_PROFILETYPE_KEYBOARDLAYOUT => manager.ActivateProfile(
                profile.dwProfileType,
                profile.langid,
                null(),
                null(),
                profile.hkl,
                flags,
            ),
            _ => Err(Error::new(
                E_FAIL,
                HSTRING::from(format!(
                    "unsupported restore profile type {}",
                    profile.dwProfileType
                )),
            )),
        }
    }
}

fn is_doubao_profile(profile: &TF_INPUTPROCESSORPROFILE) -> bool {
    profile.dwProfileType == TF_PROFILETYPE_INPUTPROCESSOR
        && profile.clsid == TIP_CLSID
        && profile.guidProfile == TIP_PROFILE_GUID
}

impl From<TF_INPUTPROCESSORPROFILE> for ActiveProfileSummary {
    fn from(profile: TF_INPUTPROCESSORPROFILE) -> Self {
        Self {
            profile_type: profile.dwProfileType,
            langid: profile.langid,
            clsid: guid_with_braces(&profile.clsid),
            profile_guid: guid_with_braces(&profile.guidProfile),
            category: guid_with_braces(&profile.catid),
            hkl: profile.hkl.0,
            enabled: profile.dwFlags & TF_IPP_FLAG_ENABLED != 0,
            active: profile.dwFlags & TF_IPP_FLAG_ACTIVE != 0,
        }
    }
}

fn input_processor_profiles() -> windows::core::Result<ITfInputProcessorProfiles> {
    unsafe {
        CoCreateInstance(
            &CLSID_TF_InputProcessorProfiles,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        )
    }
}

fn input_processor_profile_mgr() -> windows::core::Result<ITfInputProcessorProfileMgr> {
    unsafe {
        CoCreateInstance(
            &CLSID_TF_InputProcessorProfiles,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        )
    }
}

fn category_mgr() -> windows::core::Result<ITfCategoryMgr> {
    unsafe {
        CoCreateInstance(
            &CLSID_TF_CategoryMgr,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        )
    }
}

fn current_dll_path() -> windows::core::Result<String> {
    let mut module = HMODULE::default();
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR::from_raw(DllRegisterServer as *const () as *const u16),
            &mut module,
        )?;
    }

    let mut buffer = vec![0u16; 260];
    loop {
        let len = unsafe { GetModuleFileNameW(module, &mut buffer) } as usize;
        if len == 0 {
            return Err(Error::from_win32());
        }
        if len < buffer.len() {
            return Ok(String::from_utf16_lossy(&buffer[..len]));
        }
        buffer.resize(buffer.len() * 2, 0);
    }
}

fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn wide_bytes(value: &[u16]) -> &[u8] {
    unsafe { slice::from_raw_parts(value.as_ptr().cast::<u8>(), std::mem::size_of_val(value)) }
}

pub fn guid_string_with_braces(guid: &GUID) -> String {
    guid_with_braces(guid)
}

fn guid_with_braces(guid: &GUID) -> String {
    format!("{{{guid:?}}}")
}

fn clsid_key_path() -> String {
    format!("CLSID\\{}", guid_with_braces(&TIP_CLSID))
}

fn tsf_profile_key_path() -> String {
    format!(
        "SOFTWARE\\Microsoft\\CTF\\TIP\\{}\\LanguageProfile\\0x{TIP_LANGID:08x}\\{}",
        guid_with_braces(&TIP_CLSID),
        guid_with_braces(&TIP_PROFILE_GUID)
    )
}

fn is_file_not_found(error: &Error) -> bool {
    error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)
}

fn diagnostic_log(message: impl AsRef<str>) {
    let message = message.as_ref();
    let line = format!("[doubao-tsf-tip] {message}");
    let debug_line = wide_z(&line);
    unsafe {
        OutputDebugStringW(PCWSTR::from_raw(debug_line.as_ptr()));
    }

    let Some(mut log_path) = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| Some(std::env::temp_dir()))
    else {
        return;
    };
    log_path.push("DoubaoVoiceInput");
    if std::fs::create_dir_all(&log_path).is_err() {
        return;
    }
    log_path.push("tsf-tip.log");

    with_diagnostic_log_mutex(|| {
        let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        else {
            return;
        };
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let line = format!("{timestamp_ms} pid={} {message}\r\n", std::process::id());
        let _ = file.write_all(line.as_bytes());
    });
}

fn with_diagnostic_log_mutex(write_log: impl FnOnce()) {
    let mutex_name = wide_z(LOG_MUTEX_NAME);
    let mutex = unsafe { CreateMutexW(None, false, PCWSTR::from_raw(mutex_name.as_ptr())) };
    let Ok(mutex) = mutex else {
        write_log();
        return;
    };

    let wait_result = unsafe { WaitForSingleObject(mutex, LOG_MUTEX_TIMEOUT_MS) };
    let owns_mutex = wait_result == WAIT_OBJECT_0 || wait_result == WAIT_ABANDONED;

    write_log();

    if owns_mutex {
        unsafe {
            let _ = ReleaseMutex(mutex);
        }
    }
    unsafe {
        let _ = CloseHandle(mutex);
    }
}

struct ComInitGuard {
    uninitialize: bool,
}

impl ComInitGuard {
    fn initialize() -> windows::core::Result<Self> {
        match unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) } {
            Ok(()) => Ok(Self { uninitialize: true }),
            Err(error) if error.code() == RPC_E_CHANGED_MODE => {
                tracing::debug!(
                    "COM is already initialized with a different threading model; continuing"
                );
                Ok(Self {
                    uninitialize: false,
                })
            }
            Err(error) => Err(error),
        }
    }
}

impl Drop for ComInitGuard {
    fn drop(&mut self) {
        if self.uninitialize {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

struct RegKey(HKEY);

impl RegKey {
    fn create(path: &str) -> windows::core::Result<Self> {
        let path = wide_z(path);
        let mut key = HKEY::default();
        unsafe {
            RegCreateKeyExW(
                HKEY_CLASSES_ROOT,
                PCWSTR::from_raw(path.as_ptr()),
                0,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut key,
                None,
            )?;
        }
        Ok(Self(key))
    }

    fn open(root: HKEY, path: &str) -> windows::core::Result<Self> {
        let path = wide_z(path);
        let mut key = HKEY::default();
        unsafe {
            RegOpenKeyExW(root, PCWSTR::from_raw(path.as_ptr()), 0, KEY_READ, &mut key)?;
        }
        Ok(Self(key))
    }

    fn delete_tree(path: &str) -> windows::core::Result<()> {
        let path = wide_z(path);
        unsafe { RegDeleteTreeW(HKEY_CLASSES_ROOT, PCWSTR::from_raw(path.as_ptr())) }
    }

    fn set_string(&self, name: Option<&str>, value: &str) -> windows::core::Result<()> {
        let name = name.map(wide_z);
        let value = wide_z(value);
        let name = name
            .as_ref()
            .map(|name| PCWSTR::from_raw(name.as_ptr()))
            .unwrap_or_else(PCWSTR::null);

        unsafe { RegSetValueExW(self.0, name, 0, REG_SZ, Some(wide_bytes(&value))) }
    }

    fn query_string(&self, name: Option<&str>) -> windows::core::Result<Option<String>> {
        let name = name.map(wide_z);
        let name = name
            .as_ref()
            .map(|name| PCWSTR::from_raw(name.as_ptr()))
            .unwrap_or_else(PCWSTR::null);

        let mut value_type = REG_VALUE_TYPE::default();
        let mut byte_len = 0u32;
        unsafe {
            RegQueryValueExW(
                self.0,
                name,
                None,
                Some(&mut value_type),
                None,
                Some(&mut byte_len),
            )?;
        }

        if value_type != REG_SZ && value_type != REG_EXPAND_SZ {
            return Ok(None);
        }

        if byte_len == 0 {
            return Ok(Some(String::new()));
        }

        let mut buffer = vec![0u16; byte_len as usize / 2];
        unsafe {
            RegQueryValueExW(
                self.0,
                name,
                None,
                Some(&mut value_type),
                Some(buffer.as_mut_ptr().cast::<u8>()),
                Some(&mut byte_len),
            )?;
        }

        let len = buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(buffer.len());
        Ok(Some(String::from_utf16_lossy(&buffer[..len])))
    }
}

impl Drop for RegKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

impl ClassFactoryObject {
    fn new() -> Box<Self> {
        OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            vtbl: &CLASS_FACTORY_VTBL,
            ref_count: AtomicU32::new(1),
        })
    }
}

impl TextServiceObject {
    fn new() -> Box<Self> {
        OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            vtbl: &TEXT_SERVICE_VTBL,
            ref_count: AtomicU32::new(1),
            client_id: AtomicU32::new(0),
            activate_flags: AtomicU32::new(0),
            thread_mgr: Mutex::new(None),
        })
    }
}

impl KeyEventSinkObject {
    fn new(service: *mut TextServiceObject) -> Box<Self> {
        OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
        unsafe {
            text_service_add_ref(service.cast::<c_void>());
        }
        Box::new(Self {
            vtbl: &KEY_EVENT_SINK_VTBL,
            ref_count: AtomicU32::new(1),
            service,
        })
    }
}

impl EditSessionObject {
    fn new(service: *mut TextServiceObject, context: ITfContext) -> Box<Self> {
        OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
        unsafe {
            text_service_add_ref(service.cast::<c_void>());
        }
        Box::new(Self {
            vtbl: &EDIT_SESSION_VTBL,
            ref_count: AtomicU32::new(1),
            service,
            context,
        })
    }
}

impl CompositionSinkObject {
    fn new(service: *mut TextServiceObject) -> Box<Self> {
        OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
        unsafe {
            text_service_add_ref(service.cast::<c_void>());
        }
        Box::new(Self {
            vtbl: &COMPOSITION_SINK_VTBL,
            ref_count: AtomicU32::new(1),
            service,
        })
    }
}

static CLASS_FACTORY_VTBL: IClassFactory_Vtbl = IClassFactory_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: class_factory_query_interface,
        AddRef: class_factory_add_ref,
        Release: class_factory_release,
    },
    CreateInstance: class_factory_create_instance,
    LockServer: class_factory_lock_server,
};

static TEXT_SERVICE_VTBL: ITfTextInputProcessorEx_Vtbl = ITfTextInputProcessorEx_Vtbl {
    base__: ITfTextInputProcessor_Vtbl {
        base__: IUnknown_Vtbl {
            QueryInterface: text_service_query_interface,
            AddRef: text_service_add_ref,
            Release: text_service_release,
        },
        Activate: text_service_activate,
        Deactivate: text_service_deactivate,
    },
    ActivateEx: text_service_activate_ex,
};

static KEY_EVENT_SINK_VTBL: ITfKeyEventSink_Vtbl = ITfKeyEventSink_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: key_event_sink_query_interface,
        AddRef: key_event_sink_add_ref,
        Release: key_event_sink_release,
    },
    OnSetFocus: key_event_sink_on_set_focus,
    OnTestKeyDown: key_event_sink_on_test_key_down,
    OnTestKeyUp: key_event_sink_on_test_key_up,
    OnKeyDown: key_event_sink_on_key_down,
    OnKeyUp: key_event_sink_on_key_up,
    OnPreservedKey: key_event_sink_on_preserved_key,
};

static EDIT_SESSION_VTBL: ITfEditSession_Vtbl = ITfEditSession_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: edit_session_query_interface,
        AddRef: edit_session_add_ref,
        Release: edit_session_release,
    },
    DoEditSession: edit_session_do_edit_session,
};

static COMPOSITION_SINK_VTBL: ITfCompositionSink_Vtbl = ITfCompositionSink_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: composition_sink_query_interface,
        AddRef: composition_sink_add_ref,
        Release: composition_sink_release,
    },
    OnCompositionTerminated: composition_sink_on_composition_terminated,
};

unsafe extern "system" fn class_factory_query_interface(
    this: *mut c_void,
    iid: *const GUID,
    interface: *mut *mut c_void,
) -> HRESULT {
    if interface.is_null() {
        return E_POINTER;
    }
    *interface = null_mut();

    if iid.is_null() {
        return E_POINTER;
    }

    if *iid == IUnknown::IID || *iid == IClassFactory::IID {
        class_factory_add_ref(this);
        *interface = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn class_factory_add_ref(this: *mut c_void) -> u32 {
    let factory = this.cast::<ClassFactoryObject>();
    (*factory).ref_count.fetch_add(1, Ordering::SeqCst) + 1
}

unsafe extern "system" fn class_factory_release(this: *mut c_void) -> u32 {
    let factory = this.cast::<ClassFactoryObject>();
    let count = (*factory).ref_count.fetch_sub(1, Ordering::SeqCst) - 1;
    if count == 0 {
        OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
        drop(Box::from_raw(factory));
    }
    count
}

unsafe extern "system" fn class_factory_create_instance(
    _this: *mut c_void,
    outer: *mut c_void,
    iid: *const GUID,
    object: *mut *mut c_void,
) -> HRESULT {
    tracing::info!("IClassFactory::CreateInstance called");
    diagnostic_log("IClassFactory::CreateInstance called");

    if object.is_null() {
        return E_POINTER;
    }
    *object = null_mut();

    if !outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }

    let service = Box::into_raw(TextServiceObject::new()).cast::<c_void>();
    let hr = text_service_query_interface(service, iid, object);
    text_service_release(service);
    hr
}

unsafe extern "system" fn class_factory_lock_server(_this: *mut c_void, lock: BOOL) -> HRESULT {
    if lock.as_bool() {
        SERVER_LOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    } else {
        SERVER_LOCK_COUNT
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                Some(count.saturating_sub(1))
            })
            .ok();
    }
    S_OK
}

unsafe extern "system" fn text_service_query_interface(
    this: *mut c_void,
    iid: *const GUID,
    interface: *mut *mut c_void,
) -> HRESULT {
    if interface.is_null() {
        return E_POINTER;
    }
    *interface = null_mut();

    if iid.is_null() {
        return E_POINTER;
    }

    if *iid == IUnknown::IID
        || *iid == ITfTextInputProcessor::IID
        || *iid == ITfTextInputProcessorEx::IID
    {
        text_service_add_ref(this);
        *interface = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn text_service_add_ref(this: *mut c_void) -> u32 {
    let service = this.cast::<TextServiceObject>();
    (*service).ref_count.fetch_add(1, Ordering::SeqCst) + 1
}

unsafe extern "system" fn text_service_release(this: *mut c_void) -> u32 {
    let service = this.cast::<TextServiceObject>();
    let count = (*service).ref_count.fetch_sub(1, Ordering::SeqCst) - 1;
    if count == 0 {
        OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
        drop(Box::from_raw(service));
    }
    count
}

fn store_thread_mgr(service: *mut TextServiceObject, thread_mgr: *mut c_void) {
    let thread_mgr = unsafe { ITfThreadMgr::from_raw_borrowed(&thread_mgr).cloned() };
    let Some(thread_mgr) = thread_mgr else {
        diagnostic_log("ActivateEx did not provide a valid ITfThreadMgr pointer");
        return;
    };

    match unsafe { (*service).thread_mgr.lock() } {
        Ok(mut slot) => {
            *slot = Some(thread_mgr);
        }
        Err(_) => diagnostic_log("failed to store ITfThreadMgr: mutex poisoned"),
    }
}

fn clear_thread_mgr(service: *mut TextServiceObject) {
    match unsafe { (*service).thread_mgr.lock() } {
        Ok(mut slot) => {
            *slot = None;
        }
        Err(_) => diagnostic_log("failed to clear ITfThreadMgr: mutex poisoned"),
    }
}

fn advise_key_event_sink(
    service: *mut TextServiceObject,
    client_id: u32,
) -> windows::core::Result<()> {
    let thread_mgr = text_service_thread_mgr(service)?;
    let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
    let sink = unsafe {
        ITfKeyEventSink::from_raw(Box::into_raw(KeyEventSinkObject::new(service)).cast())
    };
    let result = unsafe { keystroke_mgr.AdviseKeyEventSink(client_id, &sink, BOOL(1)) };
    drop(sink);
    result
}

fn unadvise_key_event_sink(service: *mut TextServiceObject, client_id: u32) {
    if client_id == 0 {
        return;
    }

    let result = text_service_thread_mgr(service)
        .and_then(|thread_mgr| thread_mgr.cast::<ITfKeystrokeMgr>())
        .and_then(|keystroke_mgr| unsafe { keystroke_mgr.UnadviseKeyEventSink(client_id) });
    if let Err(error) = result {
        diagnostic_log(format!(
            "ITfKeystrokeMgr::UnadviseKeyEventSink failed: {error:?}"
        ));
    }
}

fn text_service_thread_mgr(service: *mut TextServiceObject) -> windows::core::Result<ITfThreadMgr> {
    let guard = unsafe { (*service).thread_mgr.lock() }.map_err(|_| {
        Error::new(
            E_FAIL,
            HSTRING::from("ITfThreadMgr mutex is poisoned in TextServiceObject"),
        )
    })?;
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| Error::new(E_FAIL, HSTRING::from("ITfThreadMgr is not active")))
}

fn request_fixed_text_composition(service: *mut TextServiceObject) -> windows::core::Result<()> {
    let client_id = unsafe { (*service).client_id.load(Ordering::SeqCst) };
    if client_id == 0 {
        return Err(Error::new(
            E_FAIL,
            HSTRING::from("cannot request edit session without an active client id"),
        ));
    }

    let thread_mgr = text_service_thread_mgr(service)?;
    let document_mgr = unsafe { thread_mgr.GetFocus()? };
    let context = unsafe { document_mgr.GetTop()? };
    let edit_session = unsafe {
        ITfEditSession::from_raw(
            Box::into_raw(EditSessionObject::new(service, context.clone())).cast(),
        )
    };
    let session_result = unsafe {
        context.RequestEditSession(client_id, &edit_session, TF_ES_SYNC | TF_ES_READWRITE)?
    };
    drop(edit_session);

    if session_result.is_err() {
        return Err(Error::new(
            session_result,
            HSTRING::from(format!(
                "fixed-text composition edit session failed: {session_result:?}"
            )),
        ));
    }

    Ok(())
}

fn commit_fixed_text_composition(
    service: *mut TextServiceObject,
    context: &ITfContext,
    edit_cookie: u32,
) -> windows::core::Result<()> {
    let mut selection = [TF_SELECTION::default()];
    let mut fetched = 0u32;
    unsafe {
        context.GetSelection(
            edit_cookie,
            TF_DEFAULT_SELECTION,
            &mut selection,
            &mut fetched,
        )?;
    }
    if fetched == 0 {
        return Err(Error::new(
            E_FAIL,
            HSTRING::from("cannot start fixed-text composition without a selection range"),
        ));
    }

    let range = selection[0]
        .range
        .as_ref()
        .cloned()
        .ok_or_else(|| Error::new(E_FAIL, HSTRING::from("selection range is null")))?;
    unsafe {
        ManuallyDrop::drop(&mut selection[0].range);
    }

    let composition_sink = unsafe {
        ITfCompositionSink::from_raw(Box::into_raw(CompositionSinkObject::new(service)).cast())
    };
    let composition_mgr: ITfContextComposition = context.cast()?;
    let composition =
        unsafe { composition_mgr.StartComposition(edit_cookie, &range, &composition_sink)? };
    drop(composition_sink);

    let composition_range = unsafe { composition.GetRange()? };
    let fixed_text: Vec<u16> = "豆包固定文本".encode_utf16().collect();
    unsafe {
        composition_range.SetText(edit_cookie, TF_ST_CORRECTION, &fixed_text)?;
        composition.EndComposition(edit_cookie)?;
    }

    diagnostic_log("fixed-text TSF composition committed");
    Ok(())
}

unsafe extern "system" fn text_service_activate(
    this: *mut c_void,
    thread_mgr: *mut c_void,
    client_id: u32,
) -> HRESULT {
    tracing::info!("ITfTextInputProcessor::Activate called (client_id={client_id})");
    diagnostic_log(format!(
        "ITfTextInputProcessor::Activate called (client_id={client_id})"
    ));
    let service = this.cast::<TextServiceObject>();
    store_thread_mgr(service, thread_mgr);
    (*service).client_id.store(client_id, Ordering::SeqCst);
    (*service).activate_flags.store(0, Ordering::SeqCst);
    if let Err(error) = advise_key_event_sink(service, client_id) {
        diagnostic_log(format!(
            "ITfKeystrokeMgr::AdviseKeyEventSink failed: {error:?}"
        ));
    }
    S_OK
}

unsafe extern "system" fn text_service_deactivate(this: *mut c_void) -> HRESULT {
    tracing::info!("ITfTextInputProcessor::Deactivate called");
    diagnostic_log("ITfTextInputProcessor::Deactivate called");
    let service = this.cast::<TextServiceObject>();
    let client_id = (*service).client_id.load(Ordering::SeqCst);
    unadvise_key_event_sink(service, client_id);
    clear_thread_mgr(service);
    (*service).client_id.store(0, Ordering::SeqCst);
    (*service).activate_flags.store(0, Ordering::SeqCst);
    S_OK
}

unsafe extern "system" fn text_service_activate_ex(
    this: *mut c_void,
    thread_mgr: *mut c_void,
    client_id: u32,
    flags: u32,
) -> HRESULT {
    tracing::info!(
        "ITfTextInputProcessorEx::ActivateEx called (client_id={client_id}, flags={flags:#x})"
    );
    diagnostic_log(format!(
        "ITfTextInputProcessorEx::ActivateEx called (client_id={client_id}, flags={flags:#x})"
    ));
    let service = this.cast::<TextServiceObject>();
    store_thread_mgr(service, thread_mgr);
    (*service).client_id.store(client_id, Ordering::SeqCst);
    (*service).activate_flags.store(flags, Ordering::SeqCst);
    if let Err(error) = advise_key_event_sink(service, client_id) {
        diagnostic_log(format!(
            "ITfKeystrokeMgr::AdviseKeyEventSink failed: {error:?}"
        ));
    }
    S_OK
}

unsafe extern "system" fn key_event_sink_query_interface(
    this: *mut c_void,
    iid: *const GUID,
    interface: *mut *mut c_void,
) -> HRESULT {
    if interface.is_null() {
        return E_POINTER;
    }
    *interface = null_mut();

    if iid.is_null() {
        return E_POINTER;
    }

    if *iid == IUnknown::IID || *iid == ITfKeyEventSink::IID {
        key_event_sink_add_ref(this);
        *interface = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn key_event_sink_add_ref(this: *mut c_void) -> u32 {
    let sink = this.cast::<KeyEventSinkObject>();
    (*sink).ref_count.fetch_add(1, Ordering::SeqCst) + 1
}

unsafe extern "system" fn key_event_sink_release(this: *mut c_void) -> u32 {
    let sink = this.cast::<KeyEventSinkObject>();
    let count = (*sink).ref_count.fetch_sub(1, Ordering::SeqCst) - 1;
    if count == 0 {
        text_service_release((*sink).service.cast::<c_void>());
        OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
        drop(Box::from_raw(sink));
    }
    count
}

unsafe extern "system" fn key_event_sink_on_set_focus(
    _this: *mut c_void,
    _foreground: BOOL,
) -> HRESULT {
    S_OK
}

unsafe extern "system" fn key_event_sink_on_test_key_down(
    _this: *mut c_void,
    _context: *mut c_void,
    wparam: WPARAM,
    _lparam: LPARAM,
    eaten: *mut BOOL,
) -> HRESULT {
    if !eaten.is_null() {
        *eaten = BOOL(is_fixed_text_test_key(wparam) as i32);
    }
    S_OK
}

unsafe extern "system" fn key_event_sink_on_test_key_up(
    _this: *mut c_void,
    _context: *mut c_void,
    _wparam: WPARAM,
    _lparam: LPARAM,
    eaten: *mut BOOL,
) -> HRESULT {
    if !eaten.is_null() {
        *eaten = BOOL(0);
    }
    S_OK
}

unsafe extern "system" fn key_event_sink_on_key_down(
    this: *mut c_void,
    _context: *mut c_void,
    wparam: WPARAM,
    _lparam: LPARAM,
    eaten: *mut BOOL,
) -> HRESULT {
    if !is_fixed_text_test_key(wparam) {
        if !eaten.is_null() {
            *eaten = BOOL(0);
        }
        return S_OK;
    }

    let sink = this.cast::<KeyEventSinkObject>();
    let result = request_fixed_text_composition((*sink).service);
    if let Err(error) = result {
        diagnostic_log(format!("fixed-text TSF composition failed: {error:?}"));
        if !eaten.is_null() {
            *eaten = BOOL(0);
        }
        return error.code();
    }

    if !eaten.is_null() {
        *eaten = BOOL(1);
    }
    S_OK
}

unsafe extern "system" fn key_event_sink_on_key_up(
    _this: *mut c_void,
    _context: *mut c_void,
    wparam: WPARAM,
    _lparam: LPARAM,
    eaten: *mut BOOL,
) -> HRESULT {
    if !eaten.is_null() {
        *eaten = BOOL(is_fixed_text_test_key(wparam) as i32);
    }
    S_OK
}

unsafe extern "system" fn key_event_sink_on_preserved_key(
    _this: *mut c_void,
    _context: *mut c_void,
    _guid: *const GUID,
    eaten: *mut BOOL,
) -> HRESULT {
    if !eaten.is_null() {
        *eaten = BOOL(0);
    }
    S_OK
}

fn is_fixed_text_test_key(wparam: WPARAM) -> bool {
    wparam.0 == VK_F6.0 as usize
}

unsafe extern "system" fn edit_session_query_interface(
    this: *mut c_void,
    iid: *const GUID,
    interface: *mut *mut c_void,
) -> HRESULT {
    if interface.is_null() {
        return E_POINTER;
    }
    *interface = null_mut();

    if iid.is_null() {
        return E_POINTER;
    }

    if *iid == IUnknown::IID || *iid == ITfEditSession::IID {
        edit_session_add_ref(this);
        *interface = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn edit_session_add_ref(this: *mut c_void) -> u32 {
    let session = this.cast::<EditSessionObject>();
    (*session).ref_count.fetch_add(1, Ordering::SeqCst) + 1
}

unsafe extern "system" fn edit_session_release(this: *mut c_void) -> u32 {
    let session = this.cast::<EditSessionObject>();
    let count = (*session).ref_count.fetch_sub(1, Ordering::SeqCst) - 1;
    if count == 0 {
        text_service_release((*session).service.cast::<c_void>());
        OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
        drop(Box::from_raw(session));
    }
    count
}

unsafe extern "system" fn edit_session_do_edit_session(this: *mut c_void, ec: u32) -> HRESULT {
    let session = this.cast::<EditSessionObject>();
    match commit_fixed_text_composition((*session).service, &(*session).context, ec) {
        Ok(()) => S_OK,
        Err(error) => {
            diagnostic_log(format!("ITfEditSession::DoEditSession failed: {error:?}"));
            error.code()
        }
    }
}

unsafe extern "system" fn composition_sink_query_interface(
    this: *mut c_void,
    iid: *const GUID,
    interface: *mut *mut c_void,
) -> HRESULT {
    if interface.is_null() {
        return E_POINTER;
    }
    *interface = null_mut();

    if iid.is_null() {
        return E_POINTER;
    }

    if *iid == IUnknown::IID || *iid == ITfCompositionSink::IID {
        composition_sink_add_ref(this);
        *interface = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn composition_sink_add_ref(this: *mut c_void) -> u32 {
    let sink = this.cast::<CompositionSinkObject>();
    (*sink).ref_count.fetch_add(1, Ordering::SeqCst) + 1
}

unsafe extern "system" fn composition_sink_release(this: *mut c_void) -> u32 {
    let sink = this.cast::<CompositionSinkObject>();
    let count = (*sink).ref_count.fetch_sub(1, Ordering::SeqCst) - 1;
    if count == 0 {
        text_service_release((*sink).service.cast::<c_void>());
        OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
        drop(Box::from_raw(sink));
    }
    count
}

unsafe extern "system" fn composition_sink_on_composition_terminated(
    _this: *mut c_void,
    _edit_cookie: u32,
    _composition: *mut c_void,
) -> HRESULT {
    diagnostic_log("ITfCompositionSink::OnCompositionTerminated called");
    S_OK
}
