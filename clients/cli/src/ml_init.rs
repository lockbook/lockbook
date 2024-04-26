use crate::ensure_account_and_root;
use cli_rs::{
    cli_error::{CliError, CliResult},
    flag::Flag,
};
use hf_hub::api::sync::Api;
use lb::Core;
use std::{convert::Infallible, path::PathBuf, str::FromStr};

pub fn start(core: &Core, model: ModelType) -> Result<(), CliError> {
    ensure_account_and_root(core)?;
    // generate_text(core, target)?;

    llm_downloader(model).map_err(|err| CliError::from(format!("{:#?}", err)))?;

    Ok(())
}

// example usage to download the specific model: lockbook ml_init --model_type=llama-3-8b
// default is mistral7b
#[derive(Debug, Clone, Copy)]
pub enum ModelType {
    Mistral7BInstruct,    // 3.86 GB
    Mistral7bInstructV02, // 3.86 GB
    Llama7bChat,          // 3.53 GB
    Llama8bChat,          // 4.37 GB
    Zephyr7bAlpha,        // 4.07 GB
}

impl Default for ModelType {
    fn default() -> Self {
        ModelType::Mistral7BInstruct
    }
}

pub fn model_flag() -> Flag<'static, ModelType> {
    Flag::new("model_type")
        .description("optional model flag; available models: mistral7b, mistral7bv2, llama-2-7b, llama-3-8b, zephyr7b, if not specified, downloads mistral7b")
        .completor(|prompt: &str| {
            Ok(["mistral7b", "mistral7bv2", "llama-2-7b", "llama-3-8b", "zephyr7b"]
                .into_iter()
                .filter(|entry| entry.starts_with(prompt))
                .map(|s| s.to_string())
                .collect())
        })
}

impl FromStr for ModelType {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // println!("Selected Model to download: {s}");
        let model = match s.to_lowercase().as_str() {
            "mistral7b" => {
                println!("mistral7b is downloading");
                ModelType::Mistral7BInstruct
            }
            "mistral7bv2" => {
                println!("mistral7bv2 is downloading");
                ModelType::Mistral7bInstructV02
            }
            "llama-2-7b" => {
                println!("llama-2-7b is downloading");
                ModelType::Llama7bChat
            }
            "llama-3-8b" => {
                println!("llama-3-8b is downloading");
                ModelType::Llama8bChat
            }
            "zephyr7b" => ModelType::Zephyr7bAlpha,

            _ => {
                eprintln!("{} is not yet supported. Falling back to mistral7b", s);
                let default = ModelType::default();

                default
            }
        };
        Ok(model)
    }
}

pub struct Config {
    model_type: ModelType,      // model path
    model_name: Option<String>, // model name
}

impl Config {
    pub fn new(model_type: ModelType) -> Self {
        Self { model_type, model_name: None }
    }

    fn model_repo(&self) -> (&str, &str) {
        match self.model_type {
            ModelType::Mistral7BInstruct => {
                ("TheBloke/Mistral-7B-Instruct-v0.1-GGUF", "mistral-7b-instruct-v0.1.Q4_K_S.gguf")
            }
            ModelType::Mistral7bInstructV02 => {
                ("TheBloke/Mistral-7B-Instruct-v0.2-GGUF", "mistral-7b-instruct-v0.2.Q4_K_S.gguf")
            }
            ModelType::Llama7bChat => {
                ("TheBloke/Llama-2-7B-Chat-GGML", "llama-2-7b-chat.ggmlv3.q4_0.bin")
            }
            ModelType::Zephyr7bAlpha => {
                ("TheBloke/zephyr-7B-alpha-GGUF", "zephyr-7b-alpha.Q4_K_M.gguf")
            }
            ModelType::Llama8bChat => {
                ("QuantFactory/Meta-Llama-3-8B-GGUF",
                "Meta-Llama-3-8B.Q4_K_S.gguf",)
            }
        }
    }


    #[warn(dead_code)]
    pub fn get_model_type(&self) -> &ModelType {
        &self.model_type
    }

    pub fn model(&self) -> CliResult<()> {
        let model_path = match &self.model_name {
            Some(config) => PathBuf::from(config),
            None => {
                let (repo, filename) = self.model_repo();
                let api = Api::new()?;
                let api = api.model(repo.to_string());
                api.get(filename).map_err(|err| {
                    CliError::from(format!("Failed to get model from API: {:#?}", err))
                })?
            }
        };
        println!("Downloaded Model!");
        /*  downloads models to /Users/{username}/.cache/huggingface/hub/models--TheBloke--Mistral-7B-Instruct-v0.1-GGUF
        to change the file location: https://huggingface.co/docs/huggingface_hub/package_reference/environment_variables#hfhome
        - Open your shell configuration file (e.g. .bashrc, .zshrc, .fishrc)
        - Add the line: export XDG_CACHE_HOME="path/to/your/cache/directory"
        - Save the file and restart your shell
        */
        println!("{}", model_path.to_string_lossy());

        Ok(())
    }
}

fn llm_downloader(model: ModelType) -> CliResult<()> {
    let config: Config = Config::new(model);
    config.model()?;
    Ok(())
}
