use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::core::{ComInterface, IUnknown, IUnknown_Vtbl, GUID, HRESULT};
use windows::Win32::Foundation::{
    BOOL, CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_NOINTERFACE, E_POINTER, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Vtbl};
use windows::Win32::System::Ole::SELFREG_E_CLASS;
use windows::Win32::UI::TextServices::{
    ITfTextInputProcessor, ITfTextInputProcessorEx, ITfTextInputProcessorEx_Vtbl,
    ITfTextInputProcessor_Vtbl,
};

pub const TIP_DESCRIPTION: &str = "Doubao Voice Input";
pub const TIP_CLSID: GUID = GUID::from_u128(0x8f5c8c59_2a4d_4ddf_8ebf_f2ab0e9b5a31);
pub const TIP_PROFILE_GUID: GUID = GUID::from_u128(0x29d0f4f7_4e0c_45fd_b287_d1e9fd9aa8d4);

static OBJECT_COUNT: AtomicU32 = AtomicU32::new(0);
static SERVER_LOCK_COUNT: AtomicU32 = AtomicU32::new(0);

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
    tracing::warn!("DllRegisterServer is not implemented yet; #4 will add registry/profile setup");
    SELFREG_E_CLASS
}

#[no_mangle]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    tracing::warn!("DllUnregisterServer is not implemented yet; #4 will add cleanup");
    SELFREG_E_CLASS
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
