use std::io::{BufRead, Write};
use std::thread::sleep;
use std::time::Duration;

use cli_rs::cli_error::{CliError, CliResult};
use lb_chat::driver::Config;
use lb_chat::{Cmd, Driver, Event, LbStore, Provider, VaultTools};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::chat::{Body, Chat};
use lb_rs::model::core_config::Config as LbConfig;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;

/// With a message: send it and stream the reply. Without: print the chat.
pub fn chat(target: String, message: String) -> CliResult<()> {
    let lb = Lb::init(LbConfig::cli_config("cli")).map_err(|e| CliError::from(e.to_string()))?;
    let user = lb
        .get_account()
        .map_err(|e| CliError::from(e.to_string()))?
        .username
        .clone();
    let file = resolve_or_create(&lb, &target)?;

    if message.trim().is_empty() {
        let bytes = lb.read_document(file.id, true)?;
        print!("{}", Chat::parse(&bytes).to_markdown());
        return Ok(());
    }

    let working_dir = {
        let path = lb.get_path_by_id(file.id)?;
        path[..path.rfind('/').map_or(0, |i| i + 1)].to_string()
    };
    let resolver_lb = lb.clone();
    let resolver_user = user.clone();
    let id = file.id;
    let config = Config {
        user,
        working_dir,
        provider: Box::new(move || {
            let bytes = resolver_lb
                .read_document(id, false)
                .map_err(|e| e.to_string())?;
            let settings = Chat::parse(&bytes).settings_for(&resolver_user);
            Provider::resolve(&resolver_lb, &settings)
        }),
    };
    let tools = VaultTools::new(lb.clone());
    let driver = Driver::spawn(LbStore { lb, id }, tools, config, || {});
    driver.send(Cmd::Say { text: message, mentions: Vec::new() });

    let mut out = std::io::stdout();
    loop {
        for event in driver.poll() {
            match event {
                Event::Delta(text) => {
                    print!("{text}");
                    out.flush()?;
                }
                Event::ToolStarted(call) => eprintln!("[{} {}]", call.name, call.args),
                Event::Ask { prompt, .. } => {
                    eprint!("{prompt} [y/N] ");
                    let mut answer = String::new();
                    let _ = std::io::stdin().lock().read_line(&mut answer);
                    let yes = answer.trim().eq_ignore_ascii_case("y");
                    driver.send(if yes { Cmd::Approve } else { Cmd::Deny });
                }
                Event::Written(entry) => {
                    if let Body::Error { text } = entry.body {
                        eprintln!("error: {text}");
                    }
                }
                Event::Lost { error, .. } => {
                    eprintln!("error: the chat could not be written: {error}")
                }
                Event::RunEnded => {
                    println!();
                    return Ok(());
                }
                Event::RunStarted => {}
            }
        }
        sleep(Duration::from_millis(20));
    }
}

fn resolve_or_create(lb: &Lb, target: &str) -> CliResult<File> {
    if let Ok(id) = target.trim().parse::<Uuid>() {
        return Ok(lb.get_file_by_id(id)?);
    }
    if let Ok(file) = lb.get_by_path(target) {
        return Ok(file);
    }
    if !target.ends_with(".chat") {
        return Err(CliError::from("a new chat's path must end in .chat"));
    }
    let file = lb.create_at_path(target)?;
    if file.file_type != FileType::Document {
        return Err(CliError::from("that path is a folder"));
    }
    Ok(file)
}
