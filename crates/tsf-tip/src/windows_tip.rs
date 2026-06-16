use std::ffi::c_void;
use std::ptr::null_mut;
use std::slice;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::core::{ComInterface, Error, IUnknown, IUnknown_Vtbl, GUID, HRESULT, PCWSTR};
use windows::Win32::Foundation::{
    BOOL, CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, ERROR_FILE_NOT_FOUND, E_NOINTERFACE,
    E_POINTER, HMODULE, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IClassFactory, IClassFactory_Vtbl,
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_EXPAND_SZ,
    REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_InputProcessorProfiles, ITfInputProcessorProfiles, ITfTextInputProcessor,
    ITfTextInputProcessorEx, ITfTextInputProcessorEx_Vtbl, ITfTextInputProcessor_Vtbl,
    TF_LANGUAGEPROFILE,
};

pub const TIP_DESCRIPTION: &str = "Doubao Voice Input";
pub const TIP_CLSID: GUID = GUID::from_u128(0x8f5c8c59_2a4d_4ddf_8ebf_f2ab0e9b5a31);
pub const TIP_PROFILE_GUID: GUID = GUID::from_u128(0x29d0f4f7_4e0c_45fd_b287_d1e9fd9aa8d4);
pub const TIP_LANGID: u16 = 0x0804; // zh-CN

const THREADING_MODEL: &str = "Apartment";
const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x80010106u32 as i32);

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
    pub tsf_profile_error: Option<String>,
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
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    tracing::info!("DllGetClassObject called");

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
    }
    result.into()
}

#[no_mangle]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    let result = unregister_server();
    if let Err(error) = &result {
        tracing::error!(?error, "DllUnregisterServer failed");
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
    tracing::info!(dll_path, "Doubao TSF TIP registered");
    Ok(())
}

pub fn unregister_server() -> windows::core::Result<()> {
    let profile_result = unregister_tsf_profile();
    let registry_result = unregister_com_server();

    profile_result?;
    registry_result?;

    tracing::info!("Doubao TSF TIP unregistered");
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
        tsf_profile_error,
    }
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
    let description = wide(TIP_DESCRIPTION);
    let icon_file = wide(dll_path);

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

fn input_processor_profiles() -> windows::core::Result<ITfInputProcessorProfiles> {
    unsafe {
        CoCreateInstance(
            &CLSID_TF_InputProcessorProfiles,
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

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}

fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn wide_bytes(value: &[u16]) -> &[u8] {
    unsafe { slice::from_raw_parts(value.as_ptr().cast::<u8>(), std::mem::size_of_val(value)) }
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

unsafe extern "system" fn text_service_activate(
    this: *mut c_void,
    _thread_mgr: *mut c_void,
    client_id: u32,
) -> HRESULT {
    tracing::info!("ITfTextInputProcessor::Activate called (client_id={client_id})");
    let service = this.cast::<TextServiceObject>();
    (*service).client_id.store(client_id, Ordering::SeqCst);
    (*service).activate_flags.store(0, Ordering::SeqCst);
    S_OK
}

unsafe extern "system" fn text_service_deactivate(this: *mut c_void) -> HRESULT {
    tracing::info!("ITfTextInputProcessor::Deactivate called");
    let service = this.cast::<TextServiceObject>();
    (*service).client_id.store(0, Ordering::SeqCst);
    (*service).activate_flags.store(0, Ordering::SeqCst);
    S_OK
}

unsafe extern "system" fn text_service_activate_ex(
    this: *mut c_void,
    _thread_mgr: *mut c_void,
    client_id: u32,
    flags: u32,
) -> HRESULT {
    tracing::info!(
        "ITfTextInputProcessorEx::ActivateEx called (client_id={client_id}, flags={flags:#x})"
    );
    let service = this.cast::<TextServiceObject>();
    (*service).client_id.store(client_id, Ordering::SeqCst);
    (*service).activate_flags.store(flags, Ordering::SeqCst);
    S_OK
}
