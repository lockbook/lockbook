use std::ptr::null_mut;

use lb_rs::model::errors::LbResult;
use lb_rs::model::work_unit::WorkUnit;
use lb_rs::service::sync::SyncStatus;

use crate::LbUuid;
use crate::ffi_utils::{carray, lb_err};
use crate::lb_c_err::LbFfiErr;

#[repr(C)]
pub struct LbSyncRes {
    pub(crate) err: *mut LbFfiErr,
    pub(crate) latest_server_ts: u64,
    pub(crate) work: LbWorkUnits,
}

#[repr(C)]
pub struct LbWorkUnits {
    pub(crate) work: *mut LbWorkUnit,
    pub(crate) len: usize,
}

#[repr(C)]
pub struct LbWorkUnit {
    pub(crate) id: LbUuid,
    pub(crate) local: bool,
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

                    new_work.push(LbWorkUnit { id: work.id().into(), local });
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
