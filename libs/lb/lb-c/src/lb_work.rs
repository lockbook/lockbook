use std::ptr::null_mut;

use lb_rs::{model::{errors::LbResult, work_unit::WorkUnit}, service::sync::SyncStatus, Uuid};

use crate::{ffi_utils::{carray, lb_err}, lb_c_err::LbFfiErr};

#[repr(C)]
pub struct LbSyncRes {
    err: *mut LbFfiErr,
    latest_server_ts: u64,
    work: LbWorkUnits,
}

#[repr(C)]
pub struct LbWorkUnits {
    work: *mut LbWorkUnit,
    len: usize,
}

#[repr(C)]
pub struct LbWorkUnit {
    id: Uuid,
    local: bool,
}

impl From<LbResult<SyncStatus>> for LbSyncRes {
    fn from(value: LbResult<SyncStatus>) -> Self {
        match value {
            Ok(work) => {
                let latest_server_ts = work.latest_server_ts;

                let mut new_work = vec![];
                for work in work.work_units {
                    let local = match work {
                        WorkUnit::LocalChange(_) => true,
                        WorkUnit::ServerChange(_) => false,
                    };

                    new_work.push(LbWorkUnit { id: work.id(), local });
                }

                let (work, len) = carray(new_work);

                Self { err: null_mut(), latest_server_ts, work: LbWorkUnits { work, len } }
            }
            Err(err) => Self {
                err: lb_err(err),
                latest_server_ts: 0,
                work: LbWorkUnits { work: null_mut(), len: 0 },
            },
        }
    }
}
