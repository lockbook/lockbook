pub async fn dispatch(lb: Arc<LbServer>, req: RpcRequest) -> LbResult<Vec<u8>> {

    let raw = req.args.unwrap_or_default();
    let payload = match req.method.as_str() {
        "create_account" => {
            let (username, api_url, welcome): (String, String, bool) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.create_account(&username, &api_url, welcome).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "import_account" => {
            let (key, maybe_url): (String, Option<String>) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.import_account(&key, maybe_url.as_deref()).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "import_account_private_key_v1" => {
            let account: crate::model::account::Account =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let res = lb.import_account_private_key_v1(account).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "import_account_private_key_v2" => {
            let (pk_bytes, api_url): ( [u8; 32], String ) =
                bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let sk = SecretKey::parse(&pk_bytes)
                .map_err(core_err_unexpected)?;
            let res = lb.import_account_private_key_v2(sk, &api_url).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "import_account_phrase" => {
            let (phrase_vec, api_url): (Vec<String>, String) = bincode::deserialize(&raw).map_err(core_err_unexpected)?;
            let slice: Vec<&str> = phrase_vec.iter().map(|s| s.as_str()).collect();
            let phrase_arr: [&str; 24] = slice
                .try_into()
                .map_err(core_err_unexpected)?;
            
            let res = lb.import_account_phrase(phrase_arr, &api_url).await?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_private_key" => {
            let res: String = lb.export_account_private_key()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_private_key_v1" => {
            let res: String = lb.export_account_private_key_v1()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_private_key_v2" => {
            let res: String = lb.export_account_private_key_v2()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_phrase" => {
            let res: String = lb.export_account_phrase()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "export_account_qr" => {
            let res: Vec<u8> = lb.export_account_qr()?;
            bincode::serialize(&res).map_err(core_err_unexpected)?
        }

        "delete_account" => {
            lb.delete_account().await?;
            bincode::serialize(&()).map_err(core_err_unexpected)?
        }

        other => {
            return Err(LbErrKind::Unexpected(format!("Unknown method: {}", other)).into())
        }
    };

    Ok(payload)
}

use std::sync::Arc;

use libsecp256k1::SecretKey;
use crate::model::errors::LbErrKind;
use crate::model::errors::{core_err_unexpected};
use crate::rpc::RpcRequest;
use crate::{LbServer,LbResult};