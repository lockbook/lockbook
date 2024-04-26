use crate::ml_init::Config;
use crate::ml_init::ModelType;

use lb::Core;
use cli_rs::{
cli_error::{CliError, CliResult},
flag::Flag};
use std::fmt::Formatter;
use std::{
    path::Path,
    fmt::{self, Display}};
use crate::{ensure_account_and_root, input::FileInput};
// use candle::quantized::{ggml_file, gguf_file};

#[derive(Debug, Clone, Copy)]
pub struct Args {
    pub temperature: f32,
    pub sample_len: i32,
    pub model: Model,
}

// pub enum Temperature {
//     T1,
//     T2,
//     T3,
//     T4,
//     T5,
//     T6,
//     T7,
//     T8,
//     T9,
//     T10
// }

#[derive(Debug, Clone, Copy)]
pub enum Model {
    Mistral7BInstruct,    // 3.86 GB
    Mistral7bInstructV02, // 3.86 GB
    Llama7bChat,          // 3.53 GB
    Llama8bChat,          // 4.37 GB
    Zephyr7bAlpha, 
}

impl Default for Model {
    fn default() -> Self {
        Model::Mistral7BInstruct
    }
}

// impl Default for Temperature {
//     fn default() -> Self {
//         Temperature::T5
//     }
// }

fn check_model(model: ModelType) -> bool {
    let base_path = "/Users/mahakanakala/.cache/huggingface/hub/models--";
    let model_name = match model {
        ModelType::Mistral7BInstruct => "TheBloke--Mistral-7B-Instruct-v0.1-GGUF",
        ModelType::Mistral7bInstructV02 => "TheBloke--Mistral-7B-Instruct-v0.2-GGUF",
        ModelType::Llama7bChat => "TheBloke--Llama-2-7B-Chat-GGML",
        ModelType::Zephyr7bAlpha => "TheBloke--zephyr-7B-alpha-GGUF",
        ModelType::Llama8bChat => "QuantFactory--Meta-Llama-3-8B-GGUF",
    };
    let full_path = format!("{}{}", base_path, model_name);
    Path::new(&full_path).exists()
}

fn check_all_models() {
    for model in &[ModelType::Mistral7BInstruct, ModelType::Mistral7bInstructV02, ModelType::Llama7bChat, ModelType::Zephyr7bAlpha, ModelType::Llama8bChat] {
        let exists = check_model(*model);
        println!("Model {:?} exists: {}", model, exists);
    }
}

// fn to display available (downloaded) models
// must search the .cache/hf-hub folder for the available models

impl Display for Model {
    fn fmt(&self, m: &mut Formatter) -> fmt::Result {
        write!(m, "{}", match self {
            Model::Mistral7BInstruct => "Mistral7BInstruct",
            Model::Mistral7bInstructV02 => "Mistral7bInstructV02",
            Model::Llama7bChat => "Llama7bChat",
            Model::Llama8bChat => "Llama8bChat",
            Model::Zephyr7bAlpha => "Zephyr7bAlpha",
        })
    }
}

impl Model {
    pub fn match_model(&self) -> Option<Model> {
        match self {
            Model::Mistral7BInstruct => Some(Model::Mistral7BInstruct),
            Model::Mistral7bInstructV02 => Some(Model::Mistral7bInstructV02),
            Model::Llama7bChat => Some(Model::Llama7bChat),
            Model::Llama8bChat => Some(Model::Llama8bChat),
            Model::Zephyr7bAlpha => Some(Model::Zephyr7bAlpha),
        }
    }
}

pub fn start_chat(core: &Core, target:FileInput) -> CliResult<()>{
    ensure_account_and_root(core)?;
    get_first_prompt(core, target)?;
    Ok(())

}

// impl Temperature {
//     pub fn value(&self) -> f32 {
//         match self {
//             Temperature::T1 => 0.1,
//             Temperature::T2 => 0.2,
//             Temperature::T3 => 0.3,
//             Temperature::T4 => 0.4,
//             Temperature::T5 => 0.5,
//             Temperature::T6 => 0.6,
//             Temperature::T7 => 0.7,
//             Temperature::T8 => 0.8,
//             Temperature::T9 => 0.9,
//             Temperature::T10 => 1.0,
//         }
//     }
// }

// pub fn temperature_flag() -> Flag<'static, Temperature>{
//     Flag::new("temperature")
//         .description("a parameter ranging from 0 (deterministic) to 1( random) that controls the randomness of LLM responses")
//         .completor(|prompt: &str| {
//             Ok(["0", "0.1", "0.2", "0.3", "0.4", "0.5", "0.6", "0.7", "0.8", "0.9", "1"]
//                 .into_iter()
//                 .filter(|entry| entry.starts_with(prompt))
//                 .map(|s| s.to_string())
//                 .collect())
//         })
// }

// pub fn sample_len_flag() -> Flag<'static, Temperature>{
//     Flag::new("sample_len")
//         .description("a parameter specifies the number of tokens to generate in the output sample")
//         .completor(|prompt: &str| {
//             Ok(["10", "100", "1000", "100000", "1000000"]
//                 .into_iter()
//                 .filter(|entry| entry.starts_with(prompt))
//                 .map(|s| s.to_string())
//                 .collect())
//         })
// }

pub fn get_first_prompt(core: &Core, target: FileInput) -> CliResult<()> {
    let f = target.find(core)?;
    let file_content = String::from_utf8(core.read_document(f.id)?)
        .map_err(|e| format!("Error reading document content: {}", e))?;

    // println!("Generated text: {}", file_content);
    let context = "You are a helpful, harmless, kind AI assistant to help the user chat with their document. "; //convert to string
    let first_prompt = context.to_string() + &file_content;
    
    println!("First Prompt: {}", first_prompt);


    println!("{}", Path::new("/Users/mahakanakala/.cache/huggingface/hub/models--TheBloke--Mistral-7B-Instruct-v0.1-GGUF").exists());
    get_prompt_response(ModelType::Mistral7BInstruct)?;
    check_all_models();

    Ok(())
}

fn get_prompt_response(model: ModelType) -> CliResult<()> {
    let config = Config::new(model);
    let start = std::time::Instant::now();
    let model_path = config.get_model_type();
    println!("Model: {:?}", model);
    // println!("{}", model_path.as_str());
    Ok(())
}

fn print_stats() ->CliResult<()> {
    let temperature = 29;
    let repeat_penalty: f32 = 10.0;
    let repeat_last_n = 10;
    // println!(
    //     "avx: {}, neon: {}, simd128: {}, f16c: {}",
    //     candle::utils::with_avx(),
    //     candle::utils::with_neon(),
    //     candle::utils::with_simd128(),
    //     candle::utils::with_f16c()
    // );
    println!(
        "temp: {:.2} repeat-penalty: {:.2} repeat-last-n: {}",
        temperature, repeat_penalty, repeat_last_n
    );
    Ok(())
}