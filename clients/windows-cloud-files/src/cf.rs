//! Thin safe wrappers over the Cloud Files API (cldapi) and the sync root registry.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use lb_rs::Uuid;
use windows::core::{HSTRING, PCWSTR, PWSTR, Result};
use windows::Storage::Provider::*;
use windows::Storage::StorageFolder;
use windows::Win32::Foundation::*;
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
use windows::Win32::Storage::CloudFilters::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub const STATUS_OK: NTSTATUS = NTSTATUS(0);
pub const STATUS_FAIL: NTSTATUS = NTSTATUS(0xC000_0001_u32 as i32);

pub fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

pub struct Handle(pub HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Opens files and directories alike, without hydrating.
pub fn open(path: &Path, write: bool) -> Result<Handle> {
    let access = if write {
        FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0
    } else {
        FILE_READ_ATTRIBUTES.0
    };
    let w = wide(path);
    let h = unsafe {
        CreateFileW(
            PCWSTR(w.as_ptr()),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )?
    };
    Ok(Handle(h))
}

fn user_sid() -> Result<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;
        let token = Handle(token);
        let mut len = 0;
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        GetTokenInformation(token.0, TokenUser, Some(buf.as_mut_ptr() as _), len, &mut len)?;
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut s)?;
        let out = s.to_string().unwrap_or_default();
        let _ = LocalFree(Some(HLOCAL(s.0 as _)));
        Ok(out)
    }
}

pub fn sync_root_id(account: &str) -> Result<HSTRING> {
    Ok(HSTRING::from(format!("Lockbook!{}!{account}", user_sid()?)))
}

pub fn register(dir: &Path, account: &str) -> Result<()> {
    let info = StorageProviderSyncRootInfo::new()?;
    info.SetId(&sync_root_id(account)?)?;
    let folder = StorageFolder::GetFolderFromPathAsync(&HSTRING::from(dir.as_os_str()))?.join()?;
    info.SetPath(&folder)?;
    info.SetDisplayNameResource(&HSTRING::from(format!("Lockbook ({account})")))?;
    info.SetIconResource(&HSTRING::from("%SystemRoot%\\system32\\imageres.dll,-1043"))?;
    info.SetHydrationPolicy(StorageProviderHydrationPolicy::Full)?;
    info.SetHydrationPolicyModifier(StorageProviderHydrationPolicyModifier::AutoDehydrationAllowed)?;
    info.SetPopulationPolicy(StorageProviderPopulationPolicy::AlwaysFull)?;
    info.SetInSyncPolicy(StorageProviderInSyncPolicy::PreserveInsyncForSyncEngine)?;
    info.SetHardlinkPolicy(StorageProviderHardlinkPolicy::None)?;
    info.SetShowSiblingsAsGroup(false)?;
    info.SetVersion(&HSTRING::from("0.1"))?;
    info.SetAllowPinning(true)?;
    StorageProviderSyncRootManager::Register(&info)
}

pub fn unregister(account: &str) -> Result<()> {
    StorageProviderSyncRootManager::Unregister(&sync_root_id(account)?)
}

pub type Callback = unsafe extern "system" fn(*const CF_CALLBACK_INFO, *const CF_CALLBACK_PARAMETERS);

pub fn connect(
    dir: &Path, fetch: Callback, delete: Callback, rename: Callback,
) -> Result<CF_CONNECTION_KEY> {
    // cldapi keeps the pointer for the connection's lifetime.
    let table: &'static [CF_CALLBACK_REGISTRATION] = Box::leak(Box::new([
        CF_CALLBACK_REGISTRATION { Type: CF_CALLBACK_TYPE_FETCH_DATA, Callback: Some(fetch) },
        CF_CALLBACK_REGISTRATION { Type: CF_CALLBACK_TYPE_NOTIFY_DELETE, Callback: Some(delete) },
        CF_CALLBACK_REGISTRATION { Type: CF_CALLBACK_TYPE_NOTIFY_RENAME, Callback: Some(rename) },
        CF_CALLBACK_REGISTRATION { Type: CF_CALLBACK_TYPE_NONE, Callback: None },
    ]));
    let w = wide(dir);
    unsafe {
        CfConnectSyncRoot(
            PCWSTR(w.as_ptr()),
            table.as_ptr(),
            None,
            CF_CONNECT_FLAG_REQUIRE_FULL_FILE_PATH,
        )
    }
}

fn metadata(size: u64, folder: bool) -> CF_FS_METADATA {
    let now = lb_now_filetime();
    CF_FS_METADATA {
        BasicInfo: FILE_BASIC_INFO {
            CreationTime: now,
            LastAccessTime: now,
            LastWriteTime: now,
            ChangeTime: now,
            FileAttributes: if folder { FILE_ATTRIBUTE_DIRECTORY.0 } else { FILE_ATTRIBUTE_NORMAL.0 },
        },
        FileSize: size as i64,
    }
}

fn lb_now_filetime() -> i64 {
    // 100ns ticks since 1601
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    (unix.as_nanos() / 100) as i64 + 116_444_736_000_000_000
}

pub fn create_placeholder(parent: &Path, name: &str, id: Uuid, folder: bool, size: u64) -> Result<()> {
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut flags = CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC;
    if folder {
        flags |= CF_PLACEHOLDER_CREATE_FLAG_DISABLE_ON_DEMAND_POPULATION;
    }
    let mut info = [CF_PLACEHOLDER_CREATE_INFO {
        RelativeFileName: PCWSTR(name.as_ptr()),
        FsMetadata: metadata(size, folder),
        FileIdentity: id.as_bytes().as_ptr() as *const c_void,
        FileIdentityLength: 16,
        Flags: flags,
        ..Default::default()
    }];
    let w = wide(parent);
    unsafe { CfCreatePlaceholders(PCWSTR(w.as_ptr()), &mut info, CF_CREATE_FLAG_NONE, None)? };
    info[0].Result.ok()
}

/// The Lockbook id and whether the item is in sync, or None for a plain file.
pub fn placeholder(path: &Path) -> Option<(Uuid, bool)> {
    let h = open(path, false).ok()?;
    let mut buf = [0u8; std::mem::size_of::<CF_PLACEHOLDER_BASIC_INFO>() + 32];
    unsafe {
        CfGetPlaceholderInfo(h.0, CF_PLACEHOLDER_INFO_BASIC, buf.as_mut_ptr() as _, buf.len() as u32, None)
            .ok()?;
        let info = &*(buf.as_ptr() as *const CF_PLACEHOLDER_BASIC_INFO);
        if info.FileIdentityLength != 16 {
            return None;
        }
        let bytes = std::slice::from_raw_parts(info.FileIdentity.as_ptr(), 16);
        Some((Uuid::from_slice(bytes).ok()?, info.InSyncState == CF_IN_SYNC_STATE_IN_SYNC))
    }
}

pub fn convert(path: &Path, id: Uuid) -> Result<()> {
    let h = open(path, true)?;
    unsafe {
        CfConvertToPlaceholder(
            h.0,
            Some(id.as_bytes().as_ptr() as _),
            16,
            CF_CONVERT_FLAG_MARK_IN_SYNC,
            None,
            None,
        )
    }
}

pub fn mark_in_sync(path: &Path) -> Result<()> {
    let h = open(path, true)?;
    unsafe { CfSetInSyncState(h.0, CF_IN_SYNC_STATE_IN_SYNC, CF_SET_IN_SYNC_FLAG_NONE, None) }
}

/// Points the placeholder at new content: drops local bytes so the next open fetches them.
pub fn replace_content(path: &Path, size: u64) -> Result<()> {
    let h = open(path, true)?;
    let meta = metadata(size, false);
    unsafe {
        CfUpdatePlaceholder(
            h.0,
            Some(&meta),
            None,
            0,
            Some(&[CF_FILE_RANGE { StartingOffset: 0, Length: -1 }]),
            CF_UPDATE_FLAG_DEHYDRATE | CF_UPDATE_FLAG_MARK_IN_SYNC,
            None,
            None,
        )
    }
}

fn op_info(info: &CF_CALLBACK_INFO, kind: CF_OPERATION_TYPE) -> CF_OPERATION_INFO {
    CF_OPERATION_INFO {
        StructSize: std::mem::size_of::<CF_OPERATION_INFO>() as u32,
        Type: kind,
        ConnectionKey: info.ConnectionKey,
        TransferKey: info.TransferKey,
        RequestKey: info.RequestKey,
        ..Default::default()
    }
}

pub fn transfer(info: &CF_CALLBACK_INFO, data: Result<&[u8]>) -> Result<()> {
    let op = op_info(info, CF_OPERATION_TYPE_TRANSFER_DATA);
    let mut params = CF_OPERATION_PARAMETERS {
        ParamSize: std::mem::size_of::<CF_OPERATION_PARAMETERS>() as u32,
        ..Default::default()
    };
    params.Anonymous.TransferData = match data {
        Ok(data) => CF_OPERATION_PARAMETERS_0_0 {
            CompletionStatus: STATUS_OK,
            Buffer: data.as_ptr() as _,
            Offset: 0,
            Length: data.len() as i64,
            ..Default::default()
        },
        Err(_) => CF_OPERATION_PARAMETERS_0_0 {
            CompletionStatus: STATUS_FAIL,
            Length: info.FileSize,
            ..Default::default()
        },
    };
    unsafe { CfExecute(&op, &mut params) }
}

pub fn ack_delete(info: &CF_CALLBACK_INFO, ok: bool) -> Result<()> {
    let op = op_info(info, CF_OPERATION_TYPE_ACK_DELETE);
    let mut params = CF_OPERATION_PARAMETERS {
        ParamSize: std::mem::size_of::<CF_OPERATION_PARAMETERS>() as u32,
        ..Default::default()
    };
    params.Anonymous.AckDelete.CompletionStatus = if ok { STATUS_OK } else { STATUS_FAIL };
    unsafe { CfExecute(&op, &mut params) }
}

pub fn ack_rename(info: &CF_CALLBACK_INFO, ok: bool) -> Result<()> {
    let op = op_info(info, CF_OPERATION_TYPE_ACK_RENAME);
    let mut params = CF_OPERATION_PARAMETERS {
        ParamSize: std::mem::size_of::<CF_OPERATION_PARAMETERS>() as u32,
        ..Default::default()
    };
    params.Anonymous.AckRename.CompletionStatus = if ok { STATUS_OK } else { STATUS_FAIL };
    unsafe { CfExecute(&op, &mut params) }
}

pub fn identity(info: &CF_CALLBACK_INFO) -> Option<Uuid> {
    if info.FileIdentityLength != 16 || info.FileIdentity.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(info.FileIdentity as *const u8, 16) };
    Uuid::from_slice(bytes).ok()
}

pub fn pcwstr(s: PCWSTR) -> String {
    if s.is_null() {
        String::new()
    } else {
        unsafe { s.to_string().unwrap_or_default() }
    }
}
