use cli_rs::cli_error::{CliResult, Exit};
use cli_rs::command::Command;
use cli_rs::parser::Cmd;
use lb_fs::fs_impl::Drive;

fn main() {
    Command::name("lb-fs")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(
            Command::name("import")
                .description("sign in and sync a lockbook account")
                .handler(import),
        )
        .subcommand(
            Command::name("mount")
                .description("start an NFS server and mount it to /tmp/lockbook")
                .handler(mount),
        )
        .subcommand(
            Command::name("fuse")
                .description("mount the desktop app's account at ~/Lockbook with FUSE (Linux)")
                .handler(fuse),
        )
        .parse()
        .exit()
}

#[tokio::main]
async fn import() -> CliResult<()> {
    Drive::import().await?;
    Ok(())
}

#[tokio::main]
async fn mount() -> CliResult<()> {
    Drive::mount().await?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn fuse() -> CliResult<()> {
    use lb_rs::model::core_config::{ClientType, Config, host_os_version};
    lb_fs::logger::init();
    let config = Config {
        writeable_path: Config::writeable_path("egui"),
        background_work: true,
        logs: false,
        client_name: "lb-fuse".into(),
        os_version: host_os_version(),
        client_type: ClientType::Ui,
        ..Default::default()
    };
    let lb = lb_rs::blocking::Lb::init(config).map_err(|e| format!("{e:?}"))?;
    let mountpoint = std::env::args()
        .nth(2)
        .map(Into::into)
        .unwrap_or_else(lb_fs::fuse::default_mountpoint);
    lb_fs::fuse::mount(lb, &mountpoint).map_err(|e| e.to_string().into())
}

#[cfg(not(target_os = "linux"))]
fn fuse() -> CliResult<()> {
    Err("FUSE mounts are Linux only".into())
}
