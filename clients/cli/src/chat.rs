use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

use cli_rs::cli_error::{CliError, CliResult};
use lb_chat::driver::Config;
use lb_chat::{Cmd, Driver, Event, LbStore, Provider, Store, VaultTools};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::chat::{Body, Chat, Mention, Settings};
use lb_rs::model::core_config::Config as LbConfig;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;

/// With a message: send it, with a note attached if one is named, and
/// stream the reply. Without: print the chat. A model or an effort is
/// remembered in the chat first.
pub fn chat(
    target: String, message: String, model: String, effort: String, attach: String,
) -> CliResult<()> {
    let lb = Lb::init(LbConfig::cli_config("cli")).map_err(|e| CliError::from(e.to_string()))?;
    let user = lb
        .get_account()
        .map_err(|e| CliError::from(e.to_string()))?
        .username
        .clone();
    let file = resolve_or_create(&lb, &target)?;
    if !model.is_empty() || !effort.is_empty() {
        choose(&lb, file.id, &user, &model, &effort)?;
    }

    if message.trim().is_empty() {
        let bytes = lb.read_document(file.id, true)?;
        print!("{}", Chat::parse(&bytes).to_markdown());
        return Ok(());
    }

    let mentions = match attach.as_str() {
        "" => Vec::new(),
        path => vec![Mention { path: path.to_string(), id: Some(lb.get_by_path(path)?.id) }],
    };
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
    driver.send(Cmd::Say { text: message, mentions });

    let mut out = std::io::stdout();
    // Thinking goes to stderr, and a line break ends it.
    let mut thinking = false;
    loop {
        for event in driver.poll() {
            if thinking && !matches!(event, Event::Thinking(_)) {
                eprintln!();
                thinking = false;
            }
            match event {
                Event::Delta(text) => {
                    print!("{text}");
                    out.flush()?;
                }
                Event::Thinking(text) => {
                    eprint!("{text}");
                    thinking = true;
                }
                Event::ToolStarted(call) => eprintln!("[{} {}]", call.name, call.args),
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

/// Remembers a model or an effort in the chat. A new model starts at its own
/// default effort, and "default" goes back to it. An effort must be one the
/// model has been shown to take.
fn choose(lb: &Lb, id: Uuid, user: &str, model: &str, effort: &str) -> CliResult<()> {
    let mut settings = Chat::parse(&lb.read_document(id, false)?).settings_for(user);
    if !model.is_empty() {
        settings.model = Some(model.to_string());
        settings.effort = None;
    }
    if !effort.is_empty() {
        settings.effort = (effort != "default").then(|| effort.to_string());
    }
    if let Some(effort) = &settings.effort {
        let asked = Settings { effort: None, ..settings.clone() };
        let provider = Provider::resolve(lb, &asked).map_err(CliError::from)?;
        let offered = provider.efforts();
        if offered.is_empty() {
            let model = &provider.model;
            return Err(CliError::from(format!("{model} has no --effort that is known to work")));
        }
        if !offered.contains(&effort.as_str()) {
            let offered = offered.join(", ");
            return Err(CliError::from(format!("{} takes --effort: {offered}", provider.model)));
        }
    }
    LbStore { lb: lb.clone(), id }
        .update(&mut |chat| chat.set_settings(user, settings.clone()))
        .map_err(CliError::from)?;
    Ok(())
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
