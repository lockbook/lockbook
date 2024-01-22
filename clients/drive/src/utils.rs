use lb_rs::Uuid;
use nfsserve::nfs::filename3;

pub fn chop(u: Uuid) -> u64 {
    u.as_u64_pair().0
}

pub fn fmt(id: u64) -> String {
    Uuid::from_u64_pair(id, 0).to_string()
}

pub fn get_string(f: &filename3) -> String {
    String::from_utf8(f.0.clone()).unwrap()
}
