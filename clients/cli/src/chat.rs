use std::collections::VecDeque;
use std::io::Write;
use std::thread::sleep;
use std::time::{Duration, Instant};

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
/// stream the reply. With a voice script: hold a spoken conversation from
/// recorded clips. Without either: print the chat. A model or an effort is
/// remembered in the chat first.
pub fn chat(
    target: String, message: String, model: String, effort: String, attach: String, voice: String,
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

    if message.trim().is_empty() && voice.trim().is_empty() {
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
        window: lb_chat::window,
    };
    let tools = VaultTools::new(lb.clone(), id);
    let driver = Driver::spawn(LbStore { lb, id }, tools, config, || {});
    if !voice.trim().is_empty() {
        return speak(&driver, &voice);
    }
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
                Event::RunStarted
                | Event::VoiceStarted
                | Event::VoiceEnded
                | Event::Audio { .. }
                | Event::Hearing(_)
                | Event::Interrupted => {}
            }
        }
        sleep(Duration::from_millis(20));
    }
}

/// What the microphone says next.
enum Step {
    /// A clip, as PCM16 mono at 24 kHz.
    Say(Vec<u8>),
    Quiet(Duration),
    /// Silence until the reply has been heard out.
    Listen,
}

/// Holds a spoken conversation from recorded clips. Each word of `script`
/// is a WAV file to say (PCM16 mono at 24 kHz), a number of seconds to stay
/// quiet, or `.` to listen until the reply has played out; the clips go up
/// at speaking pace, and the replies are played out at theirs and printed
/// as they play. After the last word the reply is heard out and the
/// session ends.
fn speak(driver: &Driver, script: &str) -> CliResult<()> {
    let mut steps: VecDeque<Step> = script
        .split_whitespace()
        .map(|word| match word {
            "." => Ok(Step::Listen),
            _ => match word.parse::<f64>() {
                Ok(seconds) => Ok(Step::Quiet(Duration::from_secs_f64(seconds))),
                Err(_) => clip(word).map(Step::Say),
            },
        })
        .collect::<Result<_, String>>()
        .map_err(CliError::from)?;
    steps.push_back(Step::Listen);

    driver.send(Cmd::StartVoice);
    let mut out = std::io::stdout();
    let mut player = Player::default();
    let mut mic: Option<Mic> = None;
    let (mut started, mut ended) = (0, 0);
    // Runs that had started when the last clip began: the reply to the
    // clip is the run after them.
    let mut before_clip = 0;
    let mut step: Option<(Step, Instant)> = None;
    let mut stopped = false;
    loop {
        for event in driver.poll() {
            match event {
                Event::VoiceStarted => mic = Some(Mic::new()),
                Event::Audio { reply, pcm } => player.push(reply, pcm.len()),
                Event::Interrupted => player.flush(),
                Event::Delta(text) => {
                    print!("{text}");
                    out.flush()?;
                }
                Event::RunStarted => started += 1,
                Event::RunEnded => {
                    ended += 1;
                    println!();
                }
                Event::ToolStarted(call) => eprintln!("[{} {}]", call.name, call.args),
                Event::Written(entry) => match entry.body {
                    Body::User { text, .. } => eprintln!("\n> {text}"),
                    Body::Assistant { interrupted: true, .. } => println!(" [interrupted]"),
                    Body::Error { text } => eprintln!("error: {text}"),
                    _ => {}
                },
                Event::Lost { error, .. } => {
                    eprintln!("error: the chat could not be written: {error}")
                }
                Event::VoiceEnded => return Ok(()),
                Event::Hearing(text) => eprint!("{text}"),
                Event::Thinking(_) => {}
            }
        }
        for (reply, ms) in player.advance() {
            driver.send(Cmd::Played { reply, ms });
        }
        if let Some(mic) = &mut mic {
            let over = match &step {
                None => true,
                Some((Step::Say(pcm), _)) => mic.sent_of_clip >= pcm.len(),
                Some((Step::Quiet(for_), since)) => since.elapsed() >= *for_,
                Some((Step::Listen, since)) => {
                    let heard_out = started > before_clip && ended == started && player.idle();
                    heard_out || since.elapsed() > Duration::from_secs(90)
                }
            };
            if over {
                mic.sent_of_clip = 0;
                step = steps.pop_front().map(|s| (s, Instant::now()));
                if let Some((Step::Say(_), _)) = &step {
                    before_clip = started;
                }
                if step.is_none() && !stopped {
                    driver.send(Cmd::Stop);
                    stopped = true;
                }
            }
            let clip = match &step {
                Some((Step::Say(pcm), _)) => Some(pcm.as_slice()),
                _ => None,
            };
            for chunk in mic.due(clip) {
                driver.send(Cmd::Audio(chunk));
            }
        }
        sleep(Duration::from_millis(20));
    }
}

const BYTES_PER_MS: usize = 48;

/// Sends audio at speaking pace: as much as the clock has passed.
struct Mic {
    since: Instant,
    sent: usize,
    sent_of_clip: usize,
}

impl Mic {
    fn new() -> Self {
        Self { since: Instant::now(), sent: 0, sent_of_clip: 0 }
    }

    /// The audio due now: the clip's next bytes, or silence.
    fn due(&mut self, clip: Option<&[u8]>) -> Vec<Vec<u8>> {
        let due = self.since.elapsed().as_millis() as usize * BYTES_PER_MS;
        let mut out = Vec::new();
        while self.sent + 100 * BYTES_PER_MS <= due {
            let chunk = match clip {
                Some(pcm) if self.sent_of_clip < pcm.len() => {
                    let end = (self.sent_of_clip + 100 * BYTES_PER_MS).min(pcm.len());
                    let chunk = pcm[self.sent_of_clip..end].to_vec();
                    self.sent_of_clip = end;
                    chunk
                }
                _ => vec![0; 100 * BYTES_PER_MS],
            };
            self.sent += 100 * BYTES_PER_MS;
            out.push(chunk);
        }
        out
    }
}

/// Plays replies out at their own pace, without a speaker.
#[derive(Default)]
struct Player {
    queue: VecDeque<(u32, usize)>,
    played: Vec<(u32, usize)>,
    last: Option<Instant>,
}

impl Player {
    fn push(&mut self, reply: u32, bytes: usize) {
        self.queue.push_back((reply, bytes));
    }

    fn flush(&mut self) {
        self.queue.clear();
    }

    fn idle(&self) -> bool {
        self.queue.is_empty()
    }

    /// Plays what the clock allows and says how far each reply has got.
    fn advance(&mut self) -> Vec<(u32, u64)> {
        let now = Instant::now();
        let mut budget = self
            .last
            .map_or(0, |last| now.duration_since(last).as_millis() as usize * BYTES_PER_MS);
        self.last = Some(now);
        let mut moved = Vec::new();
        while budget > 0 {
            let Some((reply, left)) = self.queue.front_mut() else { break };
            let played = budget.min(*left);
            *left -= played;
            budget -= played;
            let total = match self.played.iter_mut().find(|(r, _)| r == reply) {
                Some((_, total)) => {
                    *total += played;
                    *total
                }
                None => {
                    self.played.push((*reply, played));
                    played
                }
            };
            moved.retain(|(r, _)| r != reply);
            moved.push((*reply, (total / BYTES_PER_MS) as u64));
            if *left == 0 {
                self.queue.pop_front();
            }
        }
        moved
    }
}

/// The samples of a WAV file holding PCM16 mono at 24 kHz.
fn clip(path: &str) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(format!("{path} is not a WAV file"));
    }
    let (mut at, mut format) = (12, None);
    while at + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &bytes[at + 8..(at + 8 + size).min(bytes.len())];
        match &bytes[at..at + 4] {
            b"fmt " if body.len() >= 16 => {
                let field = |i: usize| u16::from_le_bytes([body[i], body[i + 1]]) as u32;
                let rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
                format = Some((field(0), field(2), rate, field(14)));
            }
            b"data" => {
                return match format {
                    Some((1, 1, 24_000, 16)) => Ok(body.to_vec()),
                    _ => Err(format!(
                        "{path} must be PCM16 mono at 24 kHz: afconvert -f WAVE -d LEI16@24000 -c 1"
                    )),
                };
            }
            _ => {}
        }
        at += 8 + size + (size & 1);
    }
    Err(format!("{path} has no audio"))
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
